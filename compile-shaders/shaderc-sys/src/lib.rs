// Copyright 2016 Google Inc.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//      http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

#![allow(non_camel_case_types)]

extern crate libc;
use libc::{c_char, c_int, c_void, size_t};

pub enum ShadercCompiler {}
pub enum ShadercCompileOptions {}
pub enum ShadercCompilationResult {}

#[repr(C)]
pub struct shaderc_include_result {
    pub source_name: *const c_char,
    pub source_name_length: size_t,
    pub content: *const c_char,
    pub content_length: size_t,
    pub user_data: *mut c_void,
}

type shaderc_include_resolve_fn = extern "C" fn(
    user_data: *mut c_void,
    requested_source: *const c_char,
    type_: c_int,
    requesting_source: *const c_char,
    include_depth: size_t,
) -> *mut shaderc_include_result;

type shaderc_include_result_release_fn =
    extern "C" fn(user_data: *mut c_void, include_result: *mut shaderc_include_result);

unsafe extern "C" {
    pub fn shaderc_compiler_initialize() -> *mut ShadercCompiler;
    pub fn shaderc_compiler_release(compiler: *mut ShadercCompiler);

    pub fn shaderc_compile_into_spv(
        compiler: *const ShadercCompiler,
        source_text: *const c_char,
        source_size: size_t,
        shader_kind: i32,
        input_file_name: *const c_char,
        entry_point_name: *const c_char,
        additional_options: *const ShadercCompileOptions,
    ) -> *mut ShadercCompilationResult;
    pub fn shaderc_compile_into_spv_assembly(
        compiler: *const ShadercCompiler,
        source_text: *const c_char,
        source_size: size_t,
        shader_kind: i32,
        input_file_name: *const c_char,
        entry_point_name: *const c_char,
        additional_options: *const ShadercCompileOptions,
    ) -> *mut ShadercCompilationResult;
    pub fn shaderc_compile_into_preprocessed_text(
        compiler: *const ShadercCompiler,
        source_text: *const c_char,
        source_size: size_t,
        shader_kind: i32,
        input_file_name: *const c_char,
        entry_point_name: *const c_char,
        additional_options: *const ShadercCompileOptions,
    ) -> *mut ShadercCompilationResult;
    pub fn shaderc_assemble_into_spv(
        compiler: *const ShadercCompiler,
        source_assembly: *const c_char,
        source_size: size_t,
        additional_options: *const ShadercCompileOptions,
    ) -> *mut ShadercCompilationResult;

    pub fn shaderc_compile_options_initialize() -> *mut ShadercCompileOptions;
    pub fn shaderc_compile_options_clone(
        options: *const ShadercCompileOptions,
    ) -> *mut ShadercCompileOptions;
    pub fn shaderc_compile_options_release(options: *mut ShadercCompileOptions);

    pub fn shaderc_compile_options_add_macro_definition(
        options: *mut ShadercCompileOptions,
        name: *const c_char,
        name_length: size_t,
        value: *const c_char,
        vaule_length: size_t,
    );
    pub fn shaderc_compile_options_set_source_language(
        options: *mut ShadercCompileOptions,
        language: i32,
    );
    pub fn shaderc_compile_options_set_generate_debug_info(options: *mut ShadercCompileOptions);
    pub fn shaderc_compile_options_set_optimization_level(
        options: *mut ShadercCompileOptions,
        level: i32,
    );
    pub fn shaderc_compile_options_set_forced_version_profile(
        options: *mut ShadercCompileOptions,
        version: c_int,
        profile: i32,
    );
    pub fn shaderc_compile_options_set_include_callbacks(
        options: *mut ShadercCompileOptions,
        resolver: shaderc_include_resolve_fn,
        result_releaser: shaderc_include_result_release_fn,
        user_data: *mut c_void,
    );
    pub fn shaderc_compile_options_set_suppress_warnings(options: *mut ShadercCompileOptions);
    pub fn shaderc_compile_options_set_warnings_as_errors(options: *mut ShadercCompileOptions);
    pub fn shaderc_compile_options_set_target_env(
        options: *mut ShadercCompileOptions,
        env: i32,
        version: u32,
    );
    pub fn shaderc_compile_options_set_target_spirv(
        options: *mut ShadercCompileOptions,
        version: i32,
    );
    pub fn shaderc_compile_options_set_limit(
        options: *mut ShadercCompileOptions,
        limit: i32,
        value: c_int,
    );
    pub fn shaderc_compile_options_set_auto_bind_uniforms(
        options: *mut ShadercCompileOptions,
        auto_bind: bool,
    );
    pub fn shaderc_compile_options_set_auto_combined_image_sampler(
        options: *mut ShadercCompileOptions,
        auto_combine: bool,
    );
    pub fn shaderc_compile_options_set_hlsl_io_mapping(
        options: *mut ShadercCompileOptions,
        hlsl_iomap: bool,
    );
    pub fn shaderc_compile_options_set_hlsl_offsets(
        options: *mut ShadercCompileOptions,
        hlsl_offsets: bool,
    );
    pub fn shaderc_compile_options_set_binding_base(
        options: *mut ShadercCompileOptions,
        resource_kind: c_int,
        base: u32,
    );
    pub fn shaderc_compile_options_set_binding_base_for_stage(
        options: *mut ShadercCompileOptions,
        shader_kind: c_int,
        resource_kind: c_int,
        base: u32,
    );
    pub fn shaderc_compile_options_set_hlsl_register_set_and_binding(
        options: *mut ShadercCompileOptions,
        register: *const c_char,
        set: *const c_char,
        binding: *const c_char,
    );
    pub fn shaderc_compile_options_set_auto_map_locations(
        options: *mut ShadercCompileOptions,
        auto_map: bool,
    );
    pub fn shaderc_compile_options_set_hlsl_register_set_and_binding_for_stage(
        options: *mut ShadercCompileOptions,
        shader_kind: c_int,
        register: *const c_char,
        set: *const c_char,
        binding: *const c_char,
    );
    pub fn shaderc_compile_options_set_hlsl_functionality1(
        options: *mut ShadercCompileOptions,
        enable: bool,
    );
    pub fn shaderc_compile_options_set_invert_y(options: *mut ShadercCompileOptions, enable: bool);
    pub fn shaderc_compile_options_set_nan_clamp(options: *mut ShadercCompileOptions, enable: bool);

    pub fn shaderc_result_release(result: *mut ShadercCompilationResult);
    pub fn shaderc_result_get_compilation_status(result: *const ShadercCompilationResult) -> i32;
    pub fn shaderc_result_get_num_errors(result: *const ShadercCompilationResult) -> size_t;
    pub fn shaderc_result_get_num_warnings(result: *const ShadercCompilationResult) -> size_t;
    pub fn shaderc_result_get_error_message(
        result: *const ShadercCompilationResult,
    ) -> *const c_char;
    pub fn shaderc_result_get_length(result: *const ShadercCompilationResult) -> size_t;
    pub fn shaderc_result_get_bytes(result: *const ShadercCompilationResult) -> *const c_char;

    pub fn shaderc_get_spv_version(version: *mut c_int, revision: *mut c_int);
    pub fn shaderc_parse_version_profile(
        str: *const c_char,
        version: *mut c_int,
        profile: *mut i32,
    ) -> bool;
}
