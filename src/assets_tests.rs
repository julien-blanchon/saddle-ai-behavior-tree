use super::*;
use crate::{AbortPolicy, BehaviorTreeBuilder, BehaviorTreeLibrary, BlackboardKeyDirection};

#[test]
fn behavior_tree_asset_round_trips_through_ron() {
    let mut builder = BehaviorTreeBuilder::new("asset_roundtrip");
    let visible = builder.bool_key(
        "target_visible",
        BlackboardKeyDirection::Input,
        false,
        Some(false),
    );
    let can_attack = builder.condition_with_watch_keys("CanAttack", "can_attack", [visible]);
    let attack = builder.action("Attack", "attack");
    let patrol = builder.action("Patrol", "patrol");
    let combat = builder.sequence("Combat", [can_attack, attack]);
    let root = builder.reactive_selector("Root", AbortPolicy::LowerPriority, [combat, patrol]);
    builder.set_root(root);

    let asset = BehaviorTreeDefinitionAsset::from(builder.build().unwrap());
    let serialized = ron::ser::to_string(&asset).unwrap();
    let decoded: BehaviorTreeDefinitionAsset = ron::de::from_str(&serialized).unwrap();

    let mut library = BehaviorTreeLibrary::default();
    let definition_id = decoded.register(&mut library).unwrap();
    let definition = library.get(definition_id).unwrap();

    assert_eq!(definition.name, "asset_roundtrip");
    assert_eq!(definition.nodes.len(), 5);
    assert_eq!(definition.root, root);
}
