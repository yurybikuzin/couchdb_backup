#[macro_export]
macro_rules! will_did {
    ($method:ident => $do_what:expr, $($body:tt)*) => {{
        let s = $do_what;
        $method!("will {}", s);
        let start = std::time::Instant::now();
        let ret = {
            $($body)*
        };
        $method!("{}, did {}", arrange_millis::get(std::time::Instant::now().duration_since(start).as_millis()), s);
        ret
    }};
}
