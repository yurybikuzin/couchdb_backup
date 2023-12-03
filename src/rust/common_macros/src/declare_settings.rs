#[macro_export]
macro_rules! declare_settings {
    (
        $( $config_fields:ident : $config_types:ty, )*
    ) => {
        declare_settings!(@parse {};
            $( $config_fields : $config_types, )*
        );
    };
    (@parse
        { $($config_body:tt)* };

        $config_field:ident : $config_type:ty,
        $( $config_fields:ident : $config_types:ty, )*
    ) => {
        paste::paste! {
            declare_settings!(@parse
                { $($config_body)*
                    pub $config_field: $config_type,
                };
                $( $config_fields : $config_types, )*
            );
        }
    };
    (@parse
        { $($config_body:tt)* };
    ) => {
        #[derive(serde::Serialize, serde::Deserialize, Debug)]
        pub struct SettingsContent {
            $($config_body)*
        }
        pub struct Settings {
            pub content: SettingsContent,
        }
        lazy_static::lazy_static! {
            pub static ref SETTINGS: std::sync::RwLock<Option<Settings>> = std::sync::RwLock::new(None);
        }
        pub fn load_settings(config_path: &std::path::Path) -> anyhow::Result<()> {
            let builder = config::Config::builder().add_source(config::File::from(config_path));
            let content = builder.build()?.try_deserialize()?;
            let settings = Settings { content };
            *(SETTINGS.write().unwrap()) = Some(settings);
            Ok(())
        }
    };
}

#[macro_export]
macro_rules! settings {
    () => {
        &(*SETTINGS.read().unwrap()).as_ref().unwrap().content
    };
    ($field:ident as let ref $ref:ident) => {
        let settings = &(*SETTINGS.read().unwrap());
        let $ref = &settings.as_ref().unwrap().content.$field;
    };
    ($field:ident) => {
        (*SETTINGS.read().unwrap()).as_ref().unwrap().content.$field
    };
    ($section:ident . $field:ident as let ref $ref:ident) => {
        let settings = &(*SETTINGS.read().unwrap());
        let $ref = &settings.as_ref().unwrap().content.$section.$field;
    };
    ($section:ident . $field:ident . $sub_field:ident as let ref $ref:ident) => {
        let settings = &(*SETTINGS.read().unwrap());
        let $ref = &settings
            .as_ref()
            .unwrap()
            .content
            .$section
            .$field
            .$sub_field;
    };
    ($section:ident . $field:ident ? . $sub_field:ident as let ref $ref:ident) => {
        let settings = &(*SETTINGS.read().unwrap());
        let $ref = &settings.as_ref().unwrap().content;
        let $ref = $ref.$section.$field.as_ref();
        let $ref = $ref.map(|field| field.$sub_field.as_ref());
    };
    ($section:ident . $field:ident ? . $sub_field:ident as let ref $ref:ident : $ty:ty) => {
        let settings = &(*SETTINGS.read().unwrap());
        let $ref = &settings.as_ref().unwrap().content;
        let $ref = $ref.$section.$field.as_ref();
        let $ref: $ty = $ref.map(|field| field.$sub_field.as_ref());
    };
    ($section:ident . $field:ident ? . $sub_field:ident as let $ref:ident) => {
        let settings = &(*SETTINGS.read().unwrap());
        let $ref = &settings.as_ref().unwrap().content;
        let $ref = $ref.$section.$field.as_ref();
        let $ref = $ref.map(|field| field.$sub_field);
    };
    ($section:ident . $field:ident) => {
        (*SETTINGS.read().unwrap())
            .as_ref()
            .unwrap()
            .content
            .$section
            .$field
    };
    ($section:ident . $field:ident . $sub_field:ident) => {
        (*SETTINGS.read().unwrap())
            .as_ref()
            .unwrap()
            .content
            .$section
            .$field
            .$sub_field
    };
    ($section:ident . $field:ident ? . $sub_field:ident) => {
        (*SETTINGS.read().unwrap())
            .as_ref()
            .unwrap()
            .content
            .$section
            .$field
            .map(|field| field.$sub_field)
    }; // ($name:ident = $from:expr) => {
       //     paste::paste!{
       //         if let Some($name) = $from {
       //             if settings!($name) != $name {
       //                 warn!(
       //                     concat!("will use ", stringify!($name), " {:?} by opt instead of ", stringify!([< $name:upper >])," = {:?} in {:?}"),
       //                     $name,
       //                     settings!($name),
       //                     env_settings!(settings_toml)
       //                 );
       //                 (*SETTINGS.write().unwrap()).as_mut().unwrap().content.$name = $name;
       //             }
       //         };
       //     }
       // }
}
