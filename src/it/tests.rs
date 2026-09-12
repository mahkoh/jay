use crate::it::test_error::TestError;
use crate::it::testrun::TestRun;
use std::future::Future;
use std::rc::Rc;

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
mod t0044_stacked_focus;
mod t0045_content_type;
mod t0046_buffer_release;
mod t0047_surface_damage;
mod t0048_frame_callback;
mod t0049_surface_damage_backend;
mod t0050_fifo;
mod t0051_pointer_warp;
mod t0052_bar;
mod t0053_theme;
mod t0054_subsurface_already_attached;
mod t0055_sm_basic;
mod t0056_sm_fullscreen;
mod t0057_sm_floating;
mod t0058_sm_parent;
mod t0059_sm_background_ws;
mod t0060_overlay;
mod t0061_container_children;
mod t0062_container_split;
mod t0063_container_mono;
mod t0064_container_move_child;
mod t0065_float_geometry;
mod t0066_float_pin;
mod t0067_container_title_offsets;
mod t0068_float_title_offsets;
mod t0069_icon_title_offsets;
mod t0070_container_title_offsets_no_icons;

pub trait TestCase: Sync {
    fn name(&self) -> &'static str;
    fn dir(&self) -> &'static str;
    fn run(&self, testrun: Rc<TestRun>) -> Box<dyn Future<Output = Result<(), TestError>>>;
}

include!(concat!(env!("OUT_DIR"), "/it_tests.rs"));
