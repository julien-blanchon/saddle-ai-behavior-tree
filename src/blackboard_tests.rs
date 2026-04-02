use super::*;

#[test]
fn typed_get_set_and_clear_work() {
    let schema = BlackboardSchema {
        keys: vec![
            BlackboardKeyDefinition {
                id: BlackboardKeyId(0),
                name: "visible".to_owned(),
                value_type: BlackboardValueType::Bool,
                direction: BlackboardKeyDirection::Input,
                required: false,
                default_value: Some(true.into()),
                description: String::new(),
            },
            BlackboardKeyDefinition {
                id: BlackboardKeyId(1),
                name: "target".to_owned(),
                value_type: BlackboardValueType::Vec3,
                direction: BlackboardKeyDirection::InOut,
                required: false,
                default_value: None,
                description: String::new(),
            },
        ],
    };
    let mut blackboard = BehaviorTreeBlackboard::from_schema(&schema);

    assert_eq!(blackboard.get_bool(BlackboardKeyId(0)), Some(true));
    assert_eq!(blackboard.get_vec3(BlackboardKeyId(1)), None);

    assert!(
        blackboard
            .set(BlackboardKeyId(1), Vec3::new(1.0, 2.0, 3.0))
            .unwrap()
    );
    assert_eq!(
        blackboard.get_vec3(BlackboardKeyId(1)),
        Some(Vec3::new(1.0, 2.0, 3.0))
    );
    assert!(blackboard.clear(BlackboardKeyId(1)).unwrap());
    assert_eq!(blackboard.get_vec3(BlackboardKeyId(1)), None);
}

#[test]
fn schema_resize_preserves_matching_values_by_name() {
    let old_schema = BlackboardSchema {
        keys: vec![
            BlackboardKeyDefinition {
                id: BlackboardKeyId(0),
                name: "alert".to_owned(),
                value_type: BlackboardValueType::Bool,
                direction: BlackboardKeyDirection::InOut,
                required: false,
                default_value: Some(false.into()),
                description: String::new(),
            },
            BlackboardKeyDefinition {
                id: BlackboardKeyId(1),
                name: "count".to_owned(),
                value_type: BlackboardValueType::Int,
                direction: BlackboardKeyDirection::Local,
                required: false,
                default_value: Some(0.into()),
                description: String::new(),
            },
        ],
    };
    let new_schema = BlackboardSchema {
        keys: vec![
            BlackboardKeyDefinition {
                id: BlackboardKeyId(0),
                name: "count".to_owned(),
                value_type: BlackboardValueType::Int,
                direction: BlackboardKeyDirection::Local,
                required: false,
                default_value: Some(1.into()),
                description: String::new(),
            },
            BlackboardKeyDefinition {
                id: BlackboardKeyId(1),
                name: "phase".to_owned(),
                value_type: BlackboardValueType::Text,
                direction: BlackboardKeyDirection::Output,
                required: false,
                default_value: Some("idle".into()),
                description: String::new(),
            },
        ],
    };
    let mut blackboard = BehaviorTreeBlackboard::from_schema(&old_schema);
    blackboard.set(BlackboardKeyId(1), 7_i32).unwrap();

    blackboard.resize_to_schema(&new_schema, true);

    assert_eq!(blackboard.get_int(BlackboardKeyId(0)), Some(7));
    assert_eq!(blackboard.get_text(BlackboardKeyId(1)), Some("idle"));
}

#[test]
fn condition_evaluation_matches_value_shape() {
    let value = BlackboardValue::Float(3.5);
    assert!(BlackboardCondition::FloatGreaterThan(3.0).evaluate(Some(&value)));
    assert!(BlackboardCondition::NotEquals(BlackboardValue::Float(2.0)).evaluate(Some(&value)));
    assert!(!BlackboardCondition::IsTrue.evaluate(Some(&value)));
    assert!(BlackboardCondition::Missing.evaluate(None));
}
