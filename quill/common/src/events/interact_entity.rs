use crate::EntityId;
use libcraft_core::{Hand, InteractionType, Vec3f};
use serde::{Deserialize, Serialize};
use crate::entities::Item;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct InteractEntityEvent {
    pub target: EntityId,
    pub ty: InteractionType,
    pub target_pos: Option<Vec3f>,
    pub hand: Option<Hand>,
    pub sneaking: bool,
}
