use {
    crate::utils::{free_list::FreeList, numcell::NumCell},
    std::{
        cell::{Cell, RefCell},
        collections::BTreeSet,
        rc::{Rc, Weak},
    },
};

#[cfg(test)]
mod tests;

linear_ids!(PageAllocIds, PageAllocId, u64);

pub const PAGE_ALLOC_PAGE_SIZE: u32 = 4096;
const SIZE_CLASSES: usize = PAGE_ALLOC_PAGE_SIZE.trailing_zeros() as usize + 1;

#[derive(Default)]
pub struct PageAllocCtx {
    ids: PageAllocIds,
}

pub struct PageAlloc {
    id: PageAllocId,
    unused_pages: FreeList<u32, 3>,
    size_classes: [SizeClass; SIZE_CLASSES],
    pages: Vec<Page>,
}

#[derive(Default)]
struct Page {
    unused_entries: FreeList<u32, 2>,
    size_class_idx: Cell<u16>,
    used_entries: NumCell<u16>,
}

#[derive(Default)]
struct SizeClass {
    free_pages: RefCell<BTreeSet<SizeClassPageInfo>>,
}

#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd)]
struct SizeClassPageInfo {
    used_entries: u16,
    page_idx: u32,
}

pub struct PageAllocEntry {
    alloc: Weak<PageAlloc>,
    alloc_id: PageAllocId,
    offset: u32,
}

impl PageAllocCtx {
    pub fn create_alloc(&self, pages: usize) -> Rc<PageAlloc> {
        Rc::new(PageAlloc {
            id: self.ids.next(),
            unused_pages: Default::default(),
            size_classes: Default::default(),
            pages: (0..pages).map(|_| Default::default()).collect(),
        })
    }
}

impl PageAlloc {
    #[cfg_attr(not(test), expect(dead_code))]
    pub fn size(&self) -> usize {
        self.pages.len() * PAGE_ALLOC_PAGE_SIZE as usize
    }

    pub fn allocate(self: &Rc<Self>, size: usize) -> Option<Rc<PageAllocEntry>> {
        let size_class_idx = size.next_power_of_two().trailing_zeros() as usize;
        let size_class = &self.size_classes[size_class_idx];
        let free_pages = &mut *size_class.free_pages.borrow_mut();
        let (page_idx, page) = 'page: {
            if let Some(info) = free_pages.pop_last() {
                let page = &self.pages[info.page_idx as usize];
                break 'page (info.page_idx as u32, page);
            }
            let page_idx = self.unused_pages.acquire();
            let Some(p) = self.pages.get(page_idx as usize) else {
                self.unused_pages.release(page_idx);
                return None;
            };
            p.size_class_idx.set(size_class_idx as u16);
            (page_idx, p)
        };
        let page_offset = page.unused_entries.acquire() << size_class_idx;
        let offset = page_idx * PAGE_ALLOC_PAGE_SIZE + page_offset;
        let used_entries = page.used_entries.add_fetch(1);
        if ((used_entries as u32) << size_class_idx) < PAGE_ALLOC_PAGE_SIZE {
            free_pages.insert(SizeClassPageInfo {
                used_entries,
                page_idx,
            });
        }
        let res = PageAllocEntry {
            alloc: Rc::downgrade(self),
            alloc_id: self.id,
            offset,
        };
        Some(Rc::new(res))
    }
}

impl PageAllocEntry {
    pub fn is_in_alloc(&self, alloc: &PageAlloc) -> bool {
        self.alloc_id == alloc.id
    }

    pub fn is_not_in_alloc(&self, alloc: &PageAlloc) -> bool {
        !self.is_in_alloc(alloc)
    }

    pub fn offset(&self) -> u32 {
        self.offset
    }
}

impl Drop for PageAllocEntry {
    fn drop(&mut self) {
        let Some(heap) = self.alloc.upgrade() else {
            return;
        };
        let page_idx = self.offset / PAGE_ALLOC_PAGE_SIZE;
        let page_offset = self.offset % PAGE_ALLOC_PAGE_SIZE;
        let page = &heap.pages[page_idx as usize];
        let size_class_idx = page.size_class_idx.get();
        page.unused_entries.release(page_offset >> size_class_idx);
        let used_entries = page.used_entries.sub_fetch(1);
        let prev_used_entries = used_entries + 1;
        let size_class = &heap.size_classes[size_class_idx as usize];
        let free_pages = &mut *size_class.free_pages.borrow_mut();
        if ((prev_used_entries as u32) << size_class_idx) < PAGE_ALLOC_PAGE_SIZE {
            free_pages.remove(&SizeClassPageInfo {
                used_entries: prev_used_entries,
                page_idx,
            });
        }
        if used_entries == 0 {
            heap.unused_pages.release(page_idx);
        } else {
            free_pages.insert(SizeClassPageInfo {
                used_entries,
                page_idx,
            });
        }
    }
}
