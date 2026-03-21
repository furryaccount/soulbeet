use dioxus::prelude::*;

#[derive(Clone, Copy)]
pub struct SearchReset(pub Signal<u32>);

#[derive(Clone, Copy)]
pub struct SearchPrefill(pub Signal<Option<(String, String)>>);
