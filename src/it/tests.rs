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

mod t2022_05_01_shm_formats;
mod t2022_05_01_window;
mod t2022_05_02_multi_window;
mod t2022_05_02_quit;
mod t2022_05_03_create_seat;
mod t2022_05_03_region;
mod t2022_05_03_subsurface;
mod t2022_05_04_fullscreen_focus;
mod t2022_05_04_map_focus;
mod t2022_05_04_set_keymap;
mod t2022_05_04_tab_focus;
mod t2022_05_05_subsurface_focus;
mod t2022_05_06_graphics_initialized;
mod t2022_05_07_container_scroll_focus;
mod t2022_05_07_scroll_partial;
mod t2022_05_07_scroll_ws;
mod t2022_05_17_click_to_active_ws;
mod t2022_05_17_remove_unused_ws;
mod t2024_03_18_alpha_modifier;
mod t2024_04_02_content_type;
mod t2024_04_02_cursor_shape;
mod t2024_04_02_dnd_focus_change;
mod t2024_04_02_double_click_float;
mod t2024_04_02_foreign_toplevel_list;
mod t2024_04_02_input_region;
mod t2024_04_02_natural_scrolling;
mod t2024_04_02_output_transform;
mod t2024_04_02_preferred_buffer_scale;
mod t2024_04_02_surface_offset;
mod t2024_04_02_syncobj;
mod t2024_04_02_top_level_restacking;
mod t2024_04_02_toplevel_suspended;
mod t2024_04_02_xdg_activation;
mod t2024_04_03_data_control;
mod t2024_04_03_float_size_memoization;
mod t2024_04_03_idle;
mod t2024_04_03_scanout_feedback;
mod t2024_04_03_toplevel_drag;
mod t2024_04_03_workspace_restoration;
mod t2024_04_04_subsurface_parent_state;
mod t2024_04_12_virtual_keyboard;
mod t2024_04_14_input_method;
mod t2024_04_18_toplevel_select;
mod t2024_04_24_destroy_registry;
mod t2024_12_26_stacked_focus;
mod t2025_08_31_buffer_release;
mod t2025_08_31_surface_damage;
mod t2025_09_03_fifo;
mod t2025_09_03_frame_callback;
mod t2025_09_03_pointer_warp;
mod t2025_09_03_surface_damage_backend;
mod t2025_12_01_bar;
mod t2025_12_29_theme;
mod t2026_03_13_subsurface_already_attached;
mod t2026_04_11_sm_background_ws;
mod t2026_04_11_sm_basic;
mod t2026_04_11_sm_floating;
mod t2026_04_11_sm_fullscreen;
mod t2026_04_11_sm_parent;
mod t2026_04_29_overlay;
mod t2026_09_11_container_children;
mod t2026_09_11_container_mono;
mod t2026_09_11_container_move_child;
mod t2026_09_11_container_split;
mod t2026_09_11_container_title_offsets;
mod t2026_09_11_float_geometry;
mod t2026_09_11_float_pin;
mod t2026_09_11_float_title_offsets;
mod t2026_09_11_icon_title_offsets;
mod t2026_09_12_container_title_offsets_no_icons;

pub trait TestCase: Sync {
    fn name(&self) -> &'static str;
    fn dir(&self) -> &'static str;
    fn run(&self, testrun: Rc<TestRun>) -> Box<dyn Future<Output = Result<(), TestError>>>;
}

include!(concat!(env!("OUT_DIR"), "/it_tests.rs"));
