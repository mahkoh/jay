use pkg_config::Config;

fn main() {
    Config::new().probe("shaderc").unwrap();
}
