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

        #[test]
        fn single() {
            crate::it::run_tests_(vec![&Test])
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
mod t0021_preferred_buffer_scale;
mod t0022_toplevel_suspended;
mod t0023_xdg_activation;
mod t0024_foreign_toplevel_list;
mod t0025_dnd_focus_change;
mod t0026_output_transform;
mod t0027_input_region;
mod t0028_top_level_restacking;
mod t0029_double_click_float;
mod t0030_cursor_shape;
mod t0031_syncobj;
mod t0032_content_type;
mod t0032_data_control;
mod t0033_float_size_memoization;
mod t0034_workspace_restoration;
mod t0035_scanout_feedback;
mod t0036_idle;
mod t0037_toplevel_drag;
mod t0038_subsurface_parent_state;
mod t0039_alpha_modifier;
mod t0040_virtual_keyboard;
mod t0041_input_method;
mod t0042_toplevel_select;
mod t0043_destroy_registry;

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
        t0021_preferred_buffer_scale,
        t0022_toplevel_suspended,
        t0023_xdg_activation,
        t0024_foreign_toplevel_list,
        t0025_dnd_focus_change,
        t0026_output_transform,
        t0027_input_region,
        t0028_top_level_restacking,
        t0029_double_click_float,
        t0030_cursor_shape,
        t0031_syncobj,
        t0032_data_control,
        t0033_float_size_memoization,
        t0034_workspace_restoration,
        t0035_scanout_feedback,
        t0036_idle,
        t0037_toplevel_drag,
        t0038_subsurface_parent_state,
        t0039_alpha_modifier,
        t0040_virtual_keyboard,
        t0041_input_method,
        t0042_toplevel_select,
        t0043_destroy_registry,
    }
}
