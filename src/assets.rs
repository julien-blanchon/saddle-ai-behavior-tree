use std::fmt::{Display, Formatter};

use bevy::asset::{AssetLoader, LoadContext, io::Reader};
use bevy::prelude::*;
use bevy::reflect::TypePath;
use serde::{Deserialize, Serialize};

use crate::definition::{BehaviorTreeDefinition, BehaviorTreeDefinitionId};
use crate::resources::BehaviorTreeLibrary;

#[derive(Asset, Clone, Debug, PartialEq, Reflect, Serialize, Deserialize)]
pub struct BehaviorTreeDefinitionAsset {
    pub definition: BehaviorTreeDefinition,
}

impl BehaviorTreeDefinitionAsset {
    pub fn register(
        &self,
        library: &mut BehaviorTreeLibrary,
    ) -> Result<BehaviorTreeDefinitionId, String> {
        library.register(self.definition.clone())
    }
}

impl From<BehaviorTreeDefinition> for BehaviorTreeDefinitionAsset {
    fn from(definition: BehaviorTreeDefinition) -> Self {
        Self { definition }
    }
}

#[derive(Default, TypePath)]
pub struct BehaviorTreeDefinitionAssetLoader;

#[derive(Debug)]
pub enum BehaviorTreeDefinitionAssetLoaderError {
    Io(std::io::Error),
    Ron(ron::error::SpannedError),
}

impl Display for BehaviorTreeDefinitionAssetLoaderError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(f, "failed to read behavior-tree asset: {error}"),
            Self::Ron(error) => write!(f, "failed to parse behavior-tree RON asset: {error}"),
        }
    }
}

impl std::error::Error for BehaviorTreeDefinitionAssetLoaderError {}

impl From<std::io::Error> for BehaviorTreeDefinitionAssetLoaderError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<ron::error::SpannedError> for BehaviorTreeDefinitionAssetLoaderError {
    fn from(value: ron::error::SpannedError) -> Self {
        Self::Ron(value)
    }
}

impl AssetLoader for BehaviorTreeDefinitionAssetLoader {
    type Asset = BehaviorTreeDefinitionAsset;
    type Settings = ();
    type Error = BehaviorTreeDefinitionAssetLoaderError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &Self::Settings,
        _load_context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        Ok(ron::de::from_bytes::<BehaviorTreeDefinitionAsset>(&bytes)?)
    }

    fn extensions(&self) -> &[&str] {
        &["bt.ron"]
    }
}

#[cfg(test)]
#[path = "assets_tests.rs"]
mod tests;
