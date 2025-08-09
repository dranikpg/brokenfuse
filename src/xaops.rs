use libc::ENOENT;

use crate::{
    effect,
    ftree::Tree,
    ftypes::{ErrNo, Ino, NodeItem},
};

pub fn get(tree: &Tree, ino: Ino, name: &str) -> Option<String> {
    match name {
        "bf.ino" => Some(format!("{}", ino)),
        "bf.stats" => {
            if let NodeItem::File(ref file) = tree.get(ino)?.item {
                Some(serde_json::to_string(&file.stats).unwrap())
            } else {
                None
            }
        }
        "bf.effect" | "bf.effect/self" => {
            Some(serde_json::to_string(&tree.get(ino)?.effects).unwrap())
        }
        "bf.effect/all" => {
            let all_effects: Vec<_> = tree
                .climb(ino as Ino)
                .map(|n| &n.effects)
                .flatten()
                .collect();
            Some(serde_json::to_string(&all_effects).unwrap())
        }
        _ => None,
    }
}
pub fn set(tree: &mut Tree, ino: Ino, name: &str, value: &str) -> Result<(), ErrNo> {
    match name {
        name if name.starts_with("bf.effect.") => {
            let name = name.strip_prefix("bf.effect.").unwrap();
            let effect = effect::DefinedEffect::create(name, value)?;
            tree.get_mut(ino).ok_or(ENOENT)?.effects.add(effect);
            Ok(())
        }
        _ => Err(ENOENT),
    }
}

pub fn remove(tree: &mut Tree, ino: Ino, name: &str) -> Option<()> {
    match name {
        "bf.effect" => {
            tree.get_mut(ino as Ino)?.effects.clear();
            Some(())
        }
        name if name.starts_with("bf.effect.") => {
            tree.get_mut(ino as Ino)?
                .effects
                .remove(name.strip_prefix("bf.effect.")?);
            Some(())
        }
        _ => None,
    }
}
