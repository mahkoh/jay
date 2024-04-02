use {
    crate::it::{test_error::TestError, testrun::TestRun},
    std::{future::Future, rc::Rc},
};

macro_rules! testcase {
    () => {
        pub struct Test;

        impl crate::it::tests::TestCase for Test {
            fn name(&self) -> &'static str {
                module_path!().strip_prefix("jay::it::tests::").unwrap()
            }

            fn dir(&self) -> &'static str {
                file!().strip_suffix(".rs").unwrap()
            }

            fn run(
                &self,
                testrun: std::rc::Rc<crate::it::testrun::TestRun>,
            ) -> Box<dyn std::future::Future<Output = crate::it::test_error::TestResult>> {
                Box::new(test(testrun))
            }
        }
    };
}

mod t0001_shm_formats;
mod t0002_window;
mod t0003_multi_window;
mod t0004_quit;
mod t0005_create_seat;
mod t0006_region;
mod t0007_subsurface;
mod t0008_map_focus;
mod t0009_tab_focus;
mod t0010_fullscreen_focus;
mod t0011_set_keymap;
mod t0012_subsurface_focus;
mod t0013_graphics_initialized;
mod t0014_container_scroll_focus;
mod t0015_scroll_partial;
mod t0016_scroll_ws;
mod t0017_remove_unused_ws;
mod t0018_click_to_active_ws;
mod t0019_natural_scrolling;
mod t0020_surface_offset;

pub trait TestCase: Sync {
    fn name(&self) -> &'static str;
    fn dir(&self) -> &'static str;
    fn run(&self, testrun: Rc<TestRun>) -> Box<dyn Future<Output = Result<(), TestError>>>;
}

pub fn tests() -> Vec<&'static dyn TestCase> {
    macro_rules! tests {
        ($($module:ident,)*) => {
            vec![
                $(
                    &$module::Test,
                )*
            ]
        }
    }

    tests! {
        t0001_shm_formats,
        t0002_window,
        t0003_multi_window,
        t0004_quit,
        t0005_create_seat,
        t0006_region,
        t0007_subsurface,
        t0008_map_focus,
        t0009_tab_focus,
        t0010_fullscreen_focus,
        t0011_set_keymap,
        t0012_subsurface_focus,
        t0013_graphics_initialized,
        t0014_container_scroll_focus,
        t0015_scroll_partial,
        t0016_scroll_ws,
        t0017_remove_unused_ws,
        t0018_click_to_active_ws,
        t0019_natural_scrolling,
        t0020_surface_offset,
    }
}
