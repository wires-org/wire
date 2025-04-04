#[macro_export]
macro_rules! function_name {
    () => {{
        fn f() {}
        fn type_name_of<T>(_: T) -> &'static str {
            std::any::type_name::<T>()
        }
        let name = type_name_of(f);
        // closure for async functions
        &name[..name.len() - 3].trim_end_matches("::{{closure}}")
    }};
}

#[macro_export]
macro_rules! get_test_path {
    () => {{
        let mut path: PathBuf = env::var("WIRE_TEST_DIR").unwrap().into();
        let full_name = $crate::function_name!();
        let function_name = full_name.split("::").last().unwrap();
        path.push(function_name);
        path
    }};
}
