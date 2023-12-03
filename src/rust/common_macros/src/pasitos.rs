// https://stackoverflow.com/questions/44160750/how-to-generate-complex-enum-variants-with-a-macro-in-rust
//
// #[allow(clippy::crate_in_macro_def)] // https://rust-lang.github.io/rust-clippy/master/index.html#crate_in_macro_def
#[allow(clippy::crate_in_macro_def)]
#[macro_export]
macro_rules! pasitos {
    (stop) => {
        STOP_COUNT.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    };
    // queue push
    ($mod:ident push_back $case:ident { $($item:tt)* }) => {
        crate::pasitos::pasos::$mod::QUEUE.write().unwrap().push_back(crate::pasitos::pasos::$mod::Arg::$case { $($item)* });
    };
    ($mod:ident push_front $case:ident { $($item:tt)* }) => {
        crate::pasitos::pasos::$mod::QUEUE.write().unwrap().push_front(crate::pasitos::pasos::$mod::Arg::$case { $($item)* });
    };
    ($mod:ident pop_front) => {
        crate::pasitos::pasos::$mod::QUEUE.write().unwrap().pop_front()
    };

    // queue state
    ($mod:ident len) => {
        crate::pasitos::pasos::$mod::QUEUE.read().unwrap().len()
    };
    ($mod:ident is_empty) => {
        pasos::$mod::QUEUE.read().unwrap().is_empty()
    };
    ($mod:ident in_progress) => {
        crate::pasitos::pasos::$mod::get_in_progress()
    };
    ($mod:ident is_idle) => {
        crate::pasitos::pasos::$mod::get_in_progress() == 0 &&
        pasos::$mod::QUEUE.read().unwrap().is_empty()
    };

    // delay
    (delay $case:ident { $($item:tt)* } for $delay_duration:expr) => {
        crate::pasitos::demoras::QUEUE.write().unwrap().push(crate::pasitos::demoras::Arg {
            delay_duration: $delay_duration,
            and_then: crate::pasitos::demoras::AndThen::$case { $($item)* }
        });
    };
    (delay forever) => {
        crate::pasitos::demoras::QUEUE.write().unwrap().push(crate::pasitos::demoras::Arg {
            delay_duration: std::time::Duration::from_secs(86400 * 365 * 100),
            and_then: crate::pasitos::demoras::AndThen::ForeverEnd
        });
    };

    // main
    (
        $fut_queue:ident,
        $run_for:ident;

        init {
            $($init:tt)*
        }
        on_complete {
            $($complete:tt)*
        }
        on_next_end {
            $($on_next_end:tt)*
        }
        demoras {
            $(
                demora $and_then_case_:ident({
                    $(
                        $and_then_field_:ident : $and_then_type_:ty
                    ,)*
                }) {
                    $( $and_then_code_:tt )*
                }
            )*
        }

        $(
            pasos $mods:ident {
                max_at_once: $max_at_once:expr;

                $(
                    paso $cases:ident({
                        $( $arg_fields:ident : $arg_types:ty, )*
                    }) -> ({
                        $( $ret_fields:ident : $ret_types:ty, )*
                    }) {
                        $($run_bodys:tt)*
                    } => sync {
                        $($process_bodys:tt)*
                    }
                )+
            }
        )+
    ) => {

        trait Sync {
            fn process(self) -> Result<()>;
        }

        lazy_static::lazy_static! {
            pub static ref STOP_COUNT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        }

        pasitos!(@delay_enum
            {}, {};
            $(
                demora $and_then_case_({
                    $(
                        $and_then_field_ : $and_then_type_
                    ,)*
                }) {
                    $($and_then_code_)*
                }
            )*
        );

        pub mod pasos {
            use super::*;

            pasitos!(@parse
                $run_for,
                $fut_queue;

                init => { $($init)* };
                on_complete => { $($complete)* };
                on_next_end => { $($on_next_end)* };

                {}, // mod_list
                {}, // select
                {}, // arg
                {}, // ret
                {}; // match

                $(
                    pasos $mods {
                        max_at_once: $max_at_once;

                        $(
                            paso $cases({
                                $( $arg_fields : $arg_types ,)*
                            }) -> ({
                                $( $ret_fields : $ret_types ,)*
                            }) {
                                $($run_bodys)*
                            } => sync {
                                $($process_bodys)*
                            }
                        )+
                    }
                )+
            );

        }
    };

    (@parse
        $run_for:ident,
        $fut_queue:ident;

        init => { $($init:tt)* };
        on_complete => { $($complete:tt)* };
        on_next_end => { $($on_next_end:tt)* };

        // mod_list
        { $($mod_list_body:tt)* },
        // select
        { $($select_body:tt)* },
        // arg
        { $($body:tt)* },
        // ret
        { $($ret_body:tt)* },
        // match
        { $($match_body:tt)* };

        pasos $mod:ident {
            max_at_once: $max_at_once:expr;

            $(
                paso $case:ident({
                    $( $arg_field:ident : $arg_type:ty ,)*
                }) -> ({
                    $( $ret_field:ident : $ret_type:ty ,)*
                }) {
                    $($run_body:tt)*
                } => sync {
                    $($process_body:tt)*
                }
            )+
        }

        $(
            pasos $mod_:ident {
                max_at_once: $max_at_once_:expr;

                $(
                    paso $case_:ident({
                        $( $arg_field_:ident : $arg_type_:ty ,)*
                    }) -> ({
                        $( $ret_field_:ident : $ret_type_:ty ,)*
                    }) {
                        $($run_body_:tt)*
                    } => sync {
                        $($process_body_:tt)*
                    }
                )+
            }
        )*
    ) => {

        pasitos!(@mod
            $run_for;
            $mod => {
                max_at_once: $max_at_once;

                $(
                    paso $case({
                        $( $arg_field : $arg_type ,)*
                    }) -> ({
                        $( $ret_field : $ret_type ,)*
                    }) {
                        $($run_body)*
                    } => sync {
                        $($process_body)*
                    }
                )+
            }
        );
        paste::paste!{
            pasitos!{@parse
                $run_for,
                $fut_queue;

                init => { $($init)* };
                on_complete => { $($complete)* };
                on_next_end => { $($on_next_end)* };

                // mod_list
                { $($mod_list_body)*
                    $mod,
                },

                // select
                { $($select_body)*
                    op::Ret::[< $mod:camel >](ret) => {
                        $mod::dec_in_progress();
                        if STOP_COUNT.load(std::sync::atomic::Ordering::SeqCst) == 0 {
                            (*ret).process()
                        } else {
                            trace!("{}: pasitos was stopped", stringify!([< $mod:camel >]));
                            Ok(())
                        }
                    },
                },

                // arg
                { $($body)*
                    [<$mod:camel>](Box<$mod::Arg>),
                },

                // ret
                { $($ret_body)*
                    [<$mod:camel>](Box<$mod::Ret>),
                },

                // match
                { $($match_body)*
                    Arg::[< $mod:camel >](arg) => {
                        let ret = $mod::run(*arg).await?;
                        Ok(Ret::[< $mod:camel >](Box::new(ret)))
                    }
                };

                $(
                    pasos $mod_ {
                        max_at_once: $max_at_once_;

                        $(
                            paso $case_({
                                $( $arg_field_ : $arg_type_ ,)*
                            }) -> ({
                                $( $ret_field_ : $ret_type_ ,)*
                            }) {
                                $($run_body_)*
                            } => sync {
                                $($process_body_)*
                            }
                        )+
                    }
                )*
            }
        }
    };

    (@parse
        $run_for:ident,
        $fut_queue:ident;

        init => { $($init:tt)* };
        on_complete => { $($complete:tt)* };
        on_next_end => { $($on_next_end:tt)* };

        // mod_list
        { $($mod_list_body:tt)* },
        // select
        { $($select_body:tt)* },
        // arg
        { $($body:tt)* },
        // ret
        { $($ret_body:tt)* },
        // match
        { $($match_body:tt)* };
    ) => {
        pub mod op {
            use super::*;
            pub enum Arg {
                Delay(Box<demoras::Arg>),
                $($body)*
            }
            pub enum Ret {
                Delay(Box<demoras::Ret>),
                $($ret_body)*
            }
            pub async fn run(arg: Arg) -> Result<Ret> {
                match arg {
                    Arg::Delay(arg) => {
                        let ret = demoras::run(*arg).await?;
                        Ok(Ret::Delay(Box::new(ret)))
                    }
                    $($match_body)*
                }
            }
        }

        pub async fn run() -> Result<()> {
            let mut $fut_queue = futures::stream::FuturesUnordered::new();

            $($init)*

            pasitos!(@push
                $fut_queue, $( $mod_list_body )*
            );
            loop {
                futures::select! {
                    ret = $fut_queue.select_next_some() => {
                        // let delay_arg =
                        match ret? {
                            op::Ret::Delay(ret) => {
                                let demoras::Ret{and_then} = *ret;
                                if STOP_COUNT.load(std::sync::atomic::Ordering::SeqCst) == 0 {
                                    and_then.process()
                                } else {
                                    trace!("Delay: pasitos was stopped");
                                    Ok(())
                                }
                            },
                            $($select_body)*
                        }?;
                        if STOP_COUNT.load(std::sync::atomic::Ordering::SeqCst) == 0 {
                            $($on_next_end)*
                            // dbg!("pasitos!(@push");
                            pasitos!(@push
                                $fut_queue, $( $mod_list_body )*
                            );
                        } else {
                            trace!("select_next_some: pasitos was stopped");
                        }
                    }
                    complete => {
                        $($complete)*
                    }
                }
            };

        }
    };

    (@push
        $fut_queue:expr, $mod:ident, $( $mods:ident, )*
    ) => {
        paste::paste! {
            let in_progress = $mod::get_in_progress();
            let max_at_once = $mod::max_at_once();
            if in_progress < max_at_once {
                let capacity = max_at_once - in_progress;
                for i in 0..capacity {
                    if let Some(item) = pasitos!($mod pop_front) {
                        $mod::inc_in_progress();
                        $fut_queue.push(op::run(op::Arg::[< $mod:camel >](Box::new(item))));
                    } else {
                        break
                    }
                }
            // } else {
            //     warn!("{}::get_in_progress()={}, {}::max_at_once={}", stringify!($mod), $mod::get_in_progress(), stringify!($mod), $mod::max_at_once());
            }
        }
        pasitos!(@push $fut_queue, $( $mods, )* );
    };

    (@push
        $fut_queue:expr,
    ) => {
        if !(*demoras::QUEUE.read().unwrap()).is_empty() {
            let mut args = Vec::<demoras::Arg>::new();
            std::mem::swap(&mut args, &mut (*demoras::QUEUE.write().unwrap()));
            for arg in args {
                $fut_queue.push(op::run(op::Arg::Delay(Box::new(arg))));
            }
        }
    };

    (@mod
        $run_for:ident;
        $mod:ident => {
            max_at_once: $max_at_once:expr;

            $(
                paso $cases:ident({
                    $( $arg_fields:ident : $arg_types:ty ,)*
                }) -> ({
                    $( $ret_fields:ident : $ret_types:ty ,)*
                }) {
                    $($run_bodys:tt)*
                } => sync {
                    $($process_bodys:tt)*
                }
            )+
        }
    ) => {
        pub mod $mod {
            use super::*;

            lazy_static::lazy_static! {
                static ref IN_PROGRESS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
            }
            pub fn get_in_progress() -> usize {
                IN_PROGRESS.load(std::sync::atomic::Ordering::SeqCst)
            }
            pub fn inc_in_progress() {
                IN_PROGRESS.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            }
            pub fn dec_in_progress() {
                IN_PROGRESS.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
            }
            pub fn max_at_once() -> usize {
                $max_at_once
            }
            lazy_static::lazy_static! {
                pub static ref QUEUE: std::sync::RwLock<std::collections::VecDeque<Arg>> = std::sync::RwLock::new(std::collections::VecDeque::new());
            }
            pasitos!(@mod_arg {};
                $(
                    paso $cases({
                        $( $arg_fields : $arg_types, )*
                    }) -> ({
                        $( $ret_fields : $ret_types, )*
                    }) {
                        $($run_bodys)*
                    } => sync {
                        $($process_bodys)*
                    }
                )+
            );
            pasitos!(@mod_ret {};
                $(
                    paso $cases({
                        $( $arg_fields : $arg_types, )*
                    }) -> ({
                        $( $ret_fields : $ret_types, )*
                    }) {
                        $($run_bodys)*
                    } => sync {
                        $($process_bodys)*
                    }
                )+
            );
            pasitos!(@mod_run
                $mod, $run_for, {};
                $(
                    paso $cases({
                        $( $arg_fields : $arg_types, )*
                    }) -> ({
                        $( $ret_fields : $ret_types, )*
                    }) {
                        $($run_bodys)*
                    } => sync {
                        $($process_bodys)*
                    }
                )+
            );
            pasitos!(@mod_process
                {};
                $(
                    paso $cases({
                        $( $arg_fields : $arg_types, )*
                    }) -> ({
                        $( $ret_fields : $ret_types, )*
                    }) {
                        $($run_bodys)*
                    } => sync {
                        $($process_bodys)*
                    }
                )+
            );
        }
    };

    (@mod_process
        { $($match_body:tt)* };
        $(
            paso $cases:ident({
                $( $arg_fields:ident : $arg_types:ty, )*
            }) -> ({
                $( $ret_fields:ident : $ret_types:ty, )*
            }) {
                $($run_bodys:tt)*
            } => sync {
                $($process_bodys:tt)*
            }
        )+
    ) => {
        pasitos!(@mod_process_case
            { $($match_body)* }, {};
            $(
                paso $cases({
                    $( $arg_fields : $arg_types ,)*
                }) -> ({
                    $( $ret_fields : $ret_types ,)*
                }) {
                    $($process_bodys)*
                } => sync {
                    $($process_bodys)*
                }
            )+
        );
    };

    (@mod_process
        { $($match_body:tt)+ };
    ) => {
        impl Sync for Ret {
            fn process(self) -> Result<()> {
                match self {
                    $($match_body)+
                };
                Ok(())
            }
        }
    };

    (@mod_process_case
        { $($match_body:tt)* },
        { $( $fields_collected:ident ,)* };
        paso $case:ident({
            $( $arg_field:ident : $arg_type:ty ,)*
        }) -> ({
            $ret_field:ident : $ret_type:ty,
            $( $ret_field_:ident : $ret_type_:ty ,)*
        }) {
            $($run_body:tt)*
        } => sync {
            $($process_body:tt)*
        }
        $(
            paso $cases:ident({
                $( $arg_fields:ident : $arg_types:ty ,)*
            }) -> ({
                $( $ret_fields:ident : $ret_types:ty ,)*
            }) {
                $($run_bodys:tt)*
            } => sync {
                $($process_bodys:tt)*
            }
        )*
    ) => {
        pasitos!(@mod_process_case
            { $($match_body)* },
            {
                $( $fields_collected ,)*
                $ret_field,
            };
            paso $case({
                $( $arg_field : $arg_type ,)*
            }) -> ({
                $( $ret_field_ : $ret_type_ ,)*
            }) {
                $($run_body)*
            } => sync {
                $($process_body)*
            }
            $(
                paso $cases({
                    $( $arg_fields : $arg_types ,)*
                }) -> ({
                    $( $ret_fields : $ret_types ,)*
                }) {
                    $($process_bodys)*
                } => sync {
                    $($process_bodys)*
                }
            )*

        );
    };
    (@mod_process_case
        { $($match_body:tt)* },
        { $( $fields_collected:ident ,)* };
        paso $case:ident({
            $( $arg_field:ident : $arg_type:ty ,)*
        }) -> ({
        }) {
            $($run_body:tt)*
        } => sync { $($process_body:tt)* }
        $(
            paso $cases:ident({
                $( $arg_fields:ident : $arg_types:ty ,)*
            }) -> ({
                $( $ret_fields:ident : $ret_types:ty ,)*
            }) {
                $($run_bodys:tt)*
            } => sync {
                $($process_bodys:tt)*
            }
        )*
    ) => {
        pasitos!(@mod_process
            {
                $($match_body)*
                Ret::$case { $($fields_collected,)* } => {
                    $($process_body)*
                },
            };
            $(
                paso $cases({
                    $( $arg_fields : $arg_types ,)*
                }) -> ({
                    $( $ret_fields : $ret_types ,)*
                }) {
                    $($process_bodys)*
                } => sync {
                    $($process_bodys)*
                }
            )*
        );
    };

    (@mod_run
        $mod:ident,
        $run_for:ident,
        { $($match_body:tt)* };
        $(
            paso $cases:ident({
                $( $arg_fields:ident : $arg_types:ty, )*
            }) -> ({
                $( $ret_fields:ident : $ret_types:ty, )*
            }) {
                $($run_bodys:tt)*
            } => sync {
                $($process_bodys:tt)*
            }
        )+
    ) => {
        pasitos!(@mod_run_case
            $mod,
            $run_for,
            { $($match_body)* }, {};
            $(
                paso $cases({
                    $( $arg_fields : $arg_types ,)*
                }) -> ({
                    $( $ret_fields : $ret_types ,)*
                }) {
                    $($run_bodys)*
                } => sync {
                    $($process_bodys)*
                }
            )+
        );
    };
    (@mod_run
        $mod:ident,
        $run_for:ident,
        { $($match_body:tt)+ };
    ) => {
        pub async fn run(arg: Arg) -> Result<Ret> {
            let $run_for = format!("{}::{}", stringify!($mod), arg);
            let ret: Ret = match arg {
                $($match_body)+
            };
            Ok(ret)
        }
    };

    (@mod_run_case
        $mod:ident,
        $run_for:ident,
        { $($match_body:tt)* },
        { $( $fields_collected:ident ,)* };
        paso $case:ident({
           $arg_field:ident : $arg_type:ty,
            $( $arg_field_:ident : $arg_type_:ty ,)*
        }) -> ({
            $( $ret_field:ident : $ret_type:ty ,)*
        }) {
            $($run_body:tt)*
        } => sync {
            $($process_body:tt)*
        }
        $(
            paso $cases:ident({
                $( $arg_fields:ident : $arg_types:ty ,)*
            }) -> ({
                $( $ret_fields:ident : $ret_types:ty ,)*
            }) {
                $($run_bodys:tt)*
            } => sync {
                $($process_bodys:tt)*
            }
        )*
    ) => {
        pasitos!(@mod_run_case
            $mod,
            $run_for,
            { $($match_body)* },
            {
                $( $fields_collected ,)*
                $arg_field,
            };
            paso $case({
                $( $arg_field_ : $arg_type_ ,)*
            }) -> ({
                $( $ret_field : $ret_type ,)*
            }) {
                $($run_body)*
            } => sync {
                $($process_body)*
            }
            $(
                paso $cases({
                    $( $arg_fields : $arg_types ,)*
                }) -> ({
                    $( $ret_fields : $ret_types ,)*
                }) {
                    $($run_bodys)*
                } => sync {
                    $($process_bodys)*
                }
            )*

        );
    };
    (@mod_run_case
        $mod:ident,
        $run_for:ident,
        { $($match_body:tt)* },
        { $( $arg_fields_collected:ident ,)* };
        paso $case:ident({}) -> ({
            $( $ret_field:ident : $ret_type:ty ,)*
        }) {
            $($run_body:tt)*
        } => sync {
            $($process_body:tt)*
        }
        $(
            paso $cases:ident({
                $( $arg_fields:ident : $arg_types:ty ,)*
            }) -> ({
                $( $ret_fields:ident : $ret_types:ty ,)*
            }) {
                $($run_bodys:tt)*
            } => sync {
                $($process_bodys:tt)*
            }
        )*
    ) => {
        pasitos!(@mod_run_case_ret
            $mod,
            $run_for,
            { $($match_body)* },
            { $( $arg_fields_collected ,)* },
            {}
            ;
            paso $case({}) -> ({
                $( $ret_field : $ret_type ,)*
            }) {
                $($run_body)*
            } => sync {
                $($process_body)*
            }
            $(
                paso $cases({
                    $( $arg_fields : $arg_types ,)*
                }) -> ({
                    $( $ret_fields : $ret_types ,)*
                }) {
                    $($run_bodys)*
                } => sync {
                    $($process_bodys)*
                }
            )*
        );
    };

    (@mod_run_case_ret
        $mod:ident,
        $run_for:ident,
        { $($match_body:tt)* },
        { $( $arg_fields_collected:ident ,)* },
        { $( $ret_fields_collected:ident ,)* };
        paso $case:ident({}) -> ({}) {
            $($run_body:tt)*
        } => sync {
            $($process_body:tt)*
        }
        $(
            paso $cases:ident({
                $( $arg_fields:ident : $arg_types:ty ,)*
            }) -> ({
                $( $ret_fields:ident : $ret_types:ty ,)*
            }) {
                $($run_bodys:tt)*
            } => sync {
                $($process_bodys:tt)*
            }
        )*
    ) => {
        pasitos!(@mod_run
            $mod,
            $run_for,
            {
                $($match_body)*
                Arg::$case { $($arg_fields_collected,)* } => {
                    will_did!(trace => $run_for.clone(), {
                        $($run_body)*;
                        let _ = $run_for;
                        Ret::$case { $($ret_fields_collected,)* }
                    })
                },
            };
            $(
                paso $cases({
                    $( $arg_fields : $arg_types ,)*
                }) -> ({
                    $( $ret_fields : $ret_types ,)*
                }) {
                    $($run_bodys)*
                } => sync {
                    $($process_bodys)*
                }
            )*
        );
    };
    (@mod_run_case_ret
        $mod:ident,
        $run_for:ident,
        { $($match_body:tt)* },
        { $( $arg_fields_collected:ident ,)* },
        { $( $ret_fields_collected:ident ,)* };
        paso $case:ident({}) -> ({
            $ret_field:ident : $ret_type:ty ,
            $( $ret_field_:ident : $ret_type_:ty ,)*
        }) {
            $($run_body:tt)*
        } => sync {
            $($process_body:tt)*
        }
        $(
            paso $cases:ident({
                $( $arg_fields:ident : $arg_types:ty ,)*
            }) -> ({
                $( $ret_fields:ident : $ret_types:ty ,)*
            }) {
                $($run_bodys:tt)*
            } => sync {
                $($process_bodys:tt)*
            }
        )*
    ) => {
        pasitos!(@mod_run_case_ret
            $mod,
            $run_for,
            { $($match_body)* },
            { $( $arg_fields_collected ,)* },
            {
                $( $ret_fields_collected ,)*
                $ret_field,
            };
            paso $case({}) -> ({
                $( $ret_field_ : $ret_type_ ,)*
            }) {
                $($run_body)*
            } =>  sync {
                $($process_body)*
            }
            $(
                paso $cases({
                    $( $arg_fields : $arg_types ,)*
                }) -> ({
                    $( $ret_fields : $ret_types ,)*
                }) {
                    $($run_bodys)*
                } => sync {
                    $($process_bodys)*
                }
            )*
        );
    };

    (@mod_arg
        { $($body:tt)* };
        paso $case:ident({
            $( $case_arg_fields:ident : $case_arg_types:ty, )*
        }) -> ({
            $( $case_ret_fields:ident : $case_ret_types:ty, )*
        }) {
            $($run_body:tt)*
        } => sync {
            $($process_body:tt)*
        }
        $(
            paso $cases:ident({
                $( $arg_fields:ident : $arg_types:ty, )*
            }) -> ({
                $( $ret_fields:ident : $ret_types:ty, )*
            }) {
                $($run_bodys:tt)*
            } => sync {
                $($process_bodys:tt)*
            }
        )*
    ) => {
        pasitos!(@mod_arg
            { $($body)*
                $case {
                    $( $case_arg_fields : $case_arg_types, )*
                },
            };
            $(
                paso $cases({
                    $( $arg_fields : $arg_types, )*
                }) -> ({
                    $( $ret_fields : $ret_types, )*
                }) {
                    $($run_bodys)*
                } => sync {
                    $($process_bodys)*
                }
            )*
        );
    };
    (@mod_arg
        { $($body:tt)* };
    ) => {
        #[derive(strum::Display)]
        pub enum Arg {
            $($body)*
        }
    };

    (@mod_ret
        { $($body:tt)* };
        paso $case:ident({
            $( $case_arg_fields:ident : $case_arg_types:ty, )*
        }) -> ({
            $( $case_ret_fields:ident : $case_ret_types:ty, )*
        }) {
            $($run_body:tt)*
        } => sync {
            $($process_body:tt)*
        }
        $(
            paso $cases:ident({
                $( $arg_fields:ident : $arg_types:ty, )*
            }) -> ({
                $( $ret_fields:ident : $ret_types:ty, )*
            }) {
                $($run_bodys:tt)*
            } => sync {
                $($process_bodys:tt)*
            }
        )*
    ) => {
        pasitos!(@mod_ret
            { $($body)*
                $case {
                    $( $case_ret_fields : $case_ret_types, )*
                },
            };
            $(
                paso $cases({
                    $( $arg_fields : $arg_types, )*
                }) -> ({
                    $( $ret_fields : $ret_types, )*
                }) {
                    $($run_bodys)*
                } => sync {
                    $($process_bodys)*
                }
            )*
        );
    };
    (@mod_ret
        { $($body:tt)* };
    ) => {
        pub enum Ret {
            $($body)*
        }
    };
    (@delay_enum
        { $($enum_body:tt)* },
        {
            $(
                $cases_:ident {
                    $(
                        $fields_:ident : $types_:ty
                    ,)*
                } => {
                    $( $codes_:tt )*
                }
            )*
        };
        demora $case:ident({
            $( $field:ident : $type:ty ,)*
        }) {
            $( $code:tt )*
        }
        $(
            demora $cases:ident({
                $( $fields:ident : $types:ty ,)*
            }) {
                $( $codes:tt )*
            }
        )*
    ) => {
        pasitos!(@delay_enum
            {
                $($enum_body)*
                $case {
                    $( $field: $type, )*
                },
            },
            {
                $(
                    $cases_ {
                        $($fields_ : $types_ ,)*
                    } => {
                        $($codes_)*
                } )*
                $case {
                    $( $field : $type ,)*
                } => {
                    $( $code )*
                }
            };
            $(
                demora $cases({
                    $($fields : $types ,)*
                }) {
                    $($codes)*
                }
            )*
        );
    };
    (@delay_enum
        { $($enum_body:tt)* },
        {
            $(
                $cases_:ident {
                    $( $fields_:ident : $types_:ty ,)*
                } => {
                    $( $codes_:tt )*
                }
            )*
        };
    ) => {
        pub mod demoras {
            use super::*;

            lazy_static::lazy_static! {
                pub static ref QUEUE: std::sync::RwLock<Vec<Arg>> = std::sync::RwLock::new(Vec::new());
            }

            pub type Delay = Option<Vec<demoras::Arg>>;

            pub struct Arg {
                pub delay_duration: tokio::time::Duration,
                pub and_then: AndThen,
            }

            pub struct Ret {
                pub and_then: AndThen,
            }
            #[derive(strum::Display)]
            pub enum AndThen {
                ForeverEnd,
                $($enum_body)*
            }

            pub async fn run(arg: Arg) -> Result<Ret> {
                let Arg {
                    delay_duration,
                    and_then,
                } = arg;
                match &and_then {
                    AndThen::ForeverEnd => {},
                    and_then => {
                        trace!(
                            "demora {} for {} ",
                            and_then,
                            arrange_millis::get(delay_duration.as_millis()),
                        );
                    }
                }
                tokio::time::sleep(delay_duration).await;
                Ok(Ret { and_then })
            }
            pasitos!(@delay_match
                {};
                $(
                    $cases_{ $( $fields_: $types_,)* } => { $( $codes_)* }
                )*
            );
        }
    };

    (@delay_match
     { $($match_body:tt)* };
        $(
            $cases:ident {
                $( $fields:ident: $types:ty ,)*
            } => { $( $codes:tt )* }
        )+
    ) => {
        pasitos!(@delay_match_field
            { $($match_body)* },
            { };
            $(
                $cases{ $( $fields : $types,)* } => { $($codes)* }
            )+
        );
    };
    (@delay_match
     { $($match_body:tt)* };) => {
        impl Sync for AndThen {
            fn process(self) -> Result<()> {
                // let ret: delay::Delay =
                match self {
                    Self::ForeverEnd => { unreachable!(); },
                    $($match_body)*
                };
                #[allow(unreachable_code)]
                {
                    Ok(())
                }
            }
        }
    };

    (@delay_match_field
        { $($match_body:tt)* },
        { $( $fields_collected:ident ,)* };
        $case:ident {
            $field:ident : $type:ty,
            $( $field_:ident : $type_:ty ,)*
        } => { $( $code:tt )* }
        $(
            $cases:ident {
                $( $fields:ident: $types:ty ,)*
            } => { $( $codes:tt )* }
        )*
    ) => {
        pasitos!(@delay_match_field
            { $($match_body)* },
            { $($fields_collected,)* $field, };
            $case { $( $field_ : $type_ ,)* } => { $($code)* }
            $(
                $cases{ $( $fields : $types,)* } => { $($codes)* }
            )*
        );
    };
    (@delay_match_field
        { $($match_body:tt)* },
        { $( $fields_collected:ident ,)* };
        $case:ident { } => { $( $code:tt )* }
        $(
            $cases:ident {
                $( $fields:ident: $types:ty ,)*
            } => { $( $codes:tt )* }
        )*
    ) => {
        pasitos!(@delay_match
            {
                $($match_body)*
                AndThen::$case { $( $fields_collected ,)* } => { $($code)* },
            };
            $(
                $cases{ $( $fields : $types,)* } => { $($codes)* }
            )*
        );
    };

}
