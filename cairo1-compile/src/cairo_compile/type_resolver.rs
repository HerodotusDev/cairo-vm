use cairo_lang_sierra::extensions::NamedType;
use cairo_lang_sierra::{
    extensions::{
        array::ArrayType, enm::EnumType, felt252::Felt252Type, snapshot::SnapshotType,
        structure::StructType,
    },
    ids::{ConcreteTypeId, GenericTypeId},
    program::{ConcreteTypeLongId, GenericArg, TypeDeclaration},
};

/// Context for resolving types.
pub struct TypeResolver<'a> {
    pub type_decl: &'a [TypeDeclaration],
}

impl TypeResolver<'_> {
    pub fn get_long_id(&self, type_id: &ConcreteTypeId) -> &ConcreteTypeLongId {
        &self.type_decl[type_id.id as usize].long_id
    }

    pub fn get_generic_id(&self, type_id: &ConcreteTypeId) -> &GenericTypeId {
        &self.get_long_id(type_id).generic_id
    }

    pub fn is_felt252_array_snapshot(&self, ty: &ConcreteTypeId) -> bool {
        let long_id = self.get_long_id(ty);
        if long_id.generic_id != SnapshotType::id() {
            return false;
        }

        let [GenericArg::Type(inner_ty)] = long_id.generic_args.as_slice() else {
            return false;
        };

        self.is_felt252_array(inner_ty)
    }

    pub fn is_felt252_array(&self, ty: &ConcreteTypeId) -> bool {
        let long_id = self.get_long_id(ty);
        if long_id.generic_id != ArrayType::id() {
            return false;
        }

        let [GenericArg::Type(element_ty)] = long_id.generic_args.as_slice() else {
            return false;
        };

        *self.get_generic_id(element_ty) == Felt252Type::id()
    }

    pub fn is_felt252_span(&self, ty: &ConcreteTypeId) -> bool {
        let long_id = self.get_long_id(ty);
        if long_id.generic_id != StructType::ID {
            return false;
        }

        let [GenericArg::UserType(_), GenericArg::Type(element_ty)] =
            long_id.generic_args.as_slice()
        else {
            return false;
        };

        self.is_felt252_array_snapshot(element_ty)
    }

    pub fn is_valid_entry_point_return_type(&self, ty: &ConcreteTypeId) -> bool {
        // The return type must be an enum with two variants: (result, error).
        let Some((result_tuple_ty, err_ty)) = self.extract_result_ty(ty) else {
            return false;
        };

        // The result variant must be a tuple with one element: Span<felt252>;
        let Some(result_ty) = self.extract_struct1(result_tuple_ty) else {
            return false;
        };
        if !self.is_felt252_span(result_ty) {
            return false;
        }

        // If the error type is Array<felt252>, it's a good error type, using the old panic
        // mechanism.
        if self.is_felt252_array(err_ty) {
            return true;
        }

        // Otherwise, the error type must be a struct with two fields: (panic, data)
        let Some((_panic_ty, err_data_ty)) = self.extract_struct2(err_ty) else {
            return false;
        };

        // The data field must be a Span<felt252>.
        self.is_felt252_array(err_data_ty)
    }

    /// Extracts types `TOk`, `TErr` from the type `Result<TOk, TErr>`.
    pub fn extract_result_ty(
        &self,
        ty: &ConcreteTypeId,
    ) -> Option<(&ConcreteTypeId, &ConcreteTypeId)> {
        let long_id = self.get_long_id(ty);
        if long_id.generic_id != EnumType::id() {
            return None;
        }
        let [GenericArg::UserType(_), GenericArg::Type(result_tuple_ty), GenericArg::Type(err_ty)] =
            long_id.generic_args.as_slice()
        else {
            return None;
        };
        Some((result_tuple_ty, err_ty))
    }

    /// Extracts type `T` from the tuple type `(T,)`.
    pub fn extract_struct1(&self, ty: &ConcreteTypeId) -> Option<&ConcreteTypeId> {
        let long_id = self.get_long_id(ty);
        if long_id.generic_id != StructType::id() {
            return None;
        }
        let [GenericArg::UserType(_), GenericArg::Type(ty0)] = long_id.generic_args.as_slice()
        else {
            return None;
        };
        Some(ty0)
    }

    /// Extracts types `T0`, `T1` from the tuple type `(T0, T1)`.
    pub fn extract_struct2(
        &self,
        ty: &ConcreteTypeId,
    ) -> Option<(&ConcreteTypeId, &ConcreteTypeId)> {
        let long_id = self.get_long_id(ty);
        if long_id.generic_id != StructType::id() {
            return None;
        }
        let [GenericArg::UserType(_), GenericArg::Type(ty0), GenericArg::Type(ty1)] =
            long_id.generic_args.as_slice()
        else {
            return None;
        };
        Some((ty0, ty1))
    }
}