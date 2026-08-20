pub const fn assert_same_layout_dont_call_directly<T, U>() {
    const {
        assert!(size_of::<T>() == size_of::<U>(), "sizes differ");
        assert!(align_of::<T>() == align_of::<U>(), "alignments differ");
    }
}
