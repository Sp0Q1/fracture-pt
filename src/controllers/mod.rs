pub mod admin;
pub mod api;
pub mod auth;
pub mod contact;
pub mod engagement;
pub mod fallback;
pub mod finding;
pub mod free_scan;
pub mod home;
pub mod invoice;
pub mod org_settings;
pub mod pages;
pub mod pentester;
pub mod report;
pub mod scan_target;
pub mod service;
pub mod subscription;
pub mod uploads;

pub use fracture_core::controllers::{
    admin as core_admin, blog, jobs, middleware, oidc, oidc_state, org,
};

/// Deserialize a form field that may be a single value or a repeated sequence.
/// HTML forms send `name=val` for one checkbox, `name=val&name=val2` for multiple.
pub fn deserialize_one_or_many<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de;

    struct OneOrMany;

    impl<'de> de::Visitor<'de> for OneOrMany {
        type Value = Vec<String>;

        fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("a string or sequence of strings")
        }

        fn visit_str<E: de::Error>(self, v: &str) -> Result<Vec<String>, E> {
            Ok(vec![v.to_owned()])
        }

        fn visit_string<E: de::Error>(self, v: String) -> Result<Vec<String>, E> {
            Ok(vec![v])
        }

        fn visit_seq<A: de::SeqAccess<'de>>(self, mut seq: A) -> Result<Vec<String>, A::Error> {
            let mut vals = Vec::new();
            while let Some(v) = seq.next_element()? {
                vals.push(v);
            }
            Ok(vals)
        }
    }

    deserializer.deserialize_any(OneOrMany)
}
