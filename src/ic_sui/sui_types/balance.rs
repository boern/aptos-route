
use crate::ic_sui::move_core_types::annotated_value::{
    MoveFieldLayout, MoveStructLayout, MoveTypeLayout,
};
use crate::ic_sui::sui_types::error::ExecutionError;
use crate::ic_sui::sui_types::sui_serde::BigInt;
use crate::ic_sui::sui_types::sui_serde::Readable;
use crate::ic_sui::sui_types::SUI_FRAMEWORK_ADDRESS;
// use move_core_types::ident_str;
use crate::ic_sui::move_core_types::identifier::IdentStr;
use crate::ic_sui::move_core_types::language_storage::{StructTag, TypeTag};
use crate::ident_str;
use serde::Deserialize;
use serde::Serialize;
use serde_with::serde_as;

// use super::TypeTag;
pub const BALANCE_MODULE_NAME: &IdentStr = ident_str!("balance");
pub const BALANCE_STRUCT_NAME: &IdentStr = ident_str!("Balance");
pub const BALANCE_CREATE_REWARDS_FUNCTION_NAME: &IdentStr = ident_str!("create_staking_rewards");
pub const BALANCE_DESTROY_REBATES_FUNCTION_NAME: &IdentStr = ident_str!("destroy_storage_rebates");

#[serde_as]
#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
pub struct Supply {
    #[serde_as(as = "Readable<BigInt<u64>, _>")]
    pub value: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
pub struct Balance {
    value: u64,
}

impl Balance {
    pub fn new(value: u64) -> Self {
        Self { value }
    }

    pub fn type_(type_param: TypeTag) -> StructTag {
        StructTag {
            address: SUI_FRAMEWORK_ADDRESS,
            module: BALANCE_MODULE_NAME.to_owned(),
            name: BALANCE_STRUCT_NAME.to_owned(),
            type_params: vec![type_param],
        }
    }

    pub fn is_balance(s: &StructTag) -> bool {
        s.address == SUI_FRAMEWORK_ADDRESS
            && s.module.as_ident_str() == BALANCE_MODULE_NAME
            && s.name.as_ident_str() == BALANCE_STRUCT_NAME
    }

    pub fn withdraw(&mut self, amount: u64) -> Result<(), ExecutionError> {
        self.value -= amount;
        Ok(())
    }

    pub fn deposit_for_safe_mode(&mut self, amount: u64) {
        self.value += amount;
    }

    pub fn value(&self) -> u64 {
        self.value
    }

    pub fn to_bcs_bytes(&self) -> Vec<u8> {
        bcs::to_bytes(&self).unwrap()
    }

    pub fn layout(type_param: TypeTag) -> MoveStructLayout {
        MoveStructLayout {
            type_: Self::type_(type_param),
            fields: Box::new(vec![MoveFieldLayout::new(
                ident_str!("value").to_owned(),
                MoveTypeLayout::U64,
            )]),
        }
    }
}
