use ahash::AHashMap;
use smol_str::SmolStr;
use std::sync::Arc;
use url::Url;

use super::types::{DeclarationItem, DeclarationRef};

pub fn index_container_variants(
    map: &mut AHashMap<SmolStr, Vec<DeclarationRef>>,
    container: &SmolStr,
    pkg: Option<&str>,
    url: &Arc<Url>,
    decl: &Arc<DeclarationItem>,
) {
    let mut add_key = |key: SmolStr| {
        map.entry(key)
            .or_default()
            .push((Arc::clone(url), Arc::clone(decl)));
    };

    add_key(container.clone());
    if let Some(p) = pkg
        && !p.is_empty()
        && !container.starts_with(p)
    {
        add_key(common::concat_smol_str!(p, ".", container));
    }

    let base = container
        .split('<')
        .next()
        .unwrap_or(container.as_str())
        .trim_end_matches('?');
    if base != container.as_str() {
        add_key(SmolStr::new(base));
        if let Some(p) = pkg
            && !p.is_empty()
            && !base.starts_with(p)
        {
            add_key(common::concat_smol_str!(p, ".", base));
        }
    }

    if let Some(short) = container.rsplit('.').next() {
        let short_clean = short
            .split('<')
            .next()
            .unwrap_or(short)
            .trim_end_matches('?');
        if short != container.as_str() {
            add_key(SmolStr::new(short));
        }
        if short_clean != short {
            add_key(SmolStr::new(short_clean));
        }
    }
}

pub fn remove_container_variants(
    map: &mut AHashMap<SmolStr, Vec<DeclarationRef>>,
    container: &SmolStr,
    pkg: Option<&str>,
    url: &Url,
) {
    let mut remove_from = |key: &str| {
        if let Some(list) = map.get_mut(key) {
            list.retain(|(u, _)| u.as_ref() != url);
            if list.is_empty() {
                map.remove(key);
            }
        }
    };
    remove_from(container.as_str());
    if let Some(p) = pkg
        && !p.is_empty()
        && !container.starts_with(p)
    {
        let full = common::concat_str!(p, ".", container);
        remove_from(&full);
    }
    let base = container
        .split('<')
        .next()
        .unwrap_or(container.as_str())
        .trim_end_matches('?');
    if base != container.as_str() {
        remove_from(base);
        if let Some(p) = pkg
            && !p.is_empty()
            && !base.starts_with(p)
        {
            let full = common::concat_str!(p, ".", base);
            remove_from(&full);
        }
    }
    if let Some(short) = container.rsplit('.').next() {
        let short_clean = short
            .split('<')
            .next()
            .unwrap_or(short)
            .trim_end_matches('?');
        if short != container.as_str() {
            remove_from(short);
        }
        if short_clean != short {
            remove_from(short_clean);
        }
    }
}
