use crate::{
    effect,
    ftree::Tree,
    ftypes::{Ino, NodeItem},
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
        "bf.effect" | "bf.effect/self" => Some(tree.get(ino)?.effects.serialize().to_string()),
        "bf.effect/all" => {
            let all_effects: Vec<_> = tree
                .climb(ino as Ino)
                .flat_map(|n| match n.effects.serialize() {
                    serde_json::Value::Array(efs) => efs,
                    _ => vec![],
                })
                .collect();
            Some(serde_json::Value::Array(all_effects).to_string())
        }
        _ => None,
    }
}
pub fn set(tree: &mut Tree, ino: Ino, name: &str, value: &str) -> Option<()> {
    match name {
        "bf.effect" => Some(()),
        name if name.starts_with("bf.effect.") => {
            let name = name.strip_prefix("bf.effect.").unwrap();
            let effect = effect::DefinedEffect::create(name, value).unwrap();
            tree.get_mut(ino).unwrap().effects.add(effect);
            Some(())
        }
        _ => None,
    }
}

pub fn remove(tree: &mut Tree, ino: Ino, name: &str) -> Option<()> {
    match name {
        "bf.effect" => {
            tree.get_mut(ino as Ino)?.effects.clear();
            Some(())
        }
        name if name.starts_with("bf.effect") => {
            tree.get_mut(ino as Ino)?
                .effects
                .remove(name.strip_prefix("bf.effect")?);
            Some(())
        }
        _ => None,
    }
}
