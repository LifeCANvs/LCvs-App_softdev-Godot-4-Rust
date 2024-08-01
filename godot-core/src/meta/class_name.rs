/*
 * Copyright (c) godot-rust; Bromeon and contributors.
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
use std::any::TypeId;
use std::collections::HashMap;
use std::ffi::CStr;
use std::fmt;
use std::hash::Hash;

use godot_ffi as sys;
use sys::Global;

use crate::builtin::*;
use crate::obj::GodotClass;

// Alternative optimizations:
// - Small-array optimization for common string lengths.
// - Use HashMap and store pre-computed hash. Would need a custom S parameter for HashMap<K, V, S>, see
//   https://doc.rust-lang.org/std/hash/trait.BuildHasher.html (the default hasher recomputes the hash repeatedly).
//
// First element (index 0) is always the empty string name, which is used for "no class".
static CLASS_NAMES: Global<Vec<StringName>> = Global::new(|| vec![StringName::default()]);
static DYNAMIC_INDEX_BY_CLASS_TYPE: Global<HashMap<TypeId, u16>> = Global::default();

/// Name of a class registered with Godot.
///
/// Holds the Godot name, not the Rust name (they sometimes differ, e.g. Godot `CSGMesh3D` vs Rust `CsgMesh3D`).
///
/// This struct is very cheap to copy. The actual names are cached globally.
///
/// If you need to create your own class name, use [`new_cached()`][Self::new_cached].
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct ClassName {
    global_index: u16,
}

impl ClassName {
    /// Construct a new ASCII class name.
    ///
    /// This is expensive the first time it called for a given `T`, but will be cached for subsequent calls.
    ///
    /// It is not specified when exactly `init_fn` is invoked. However, it must return the same value for the same `T`. Generally, we expect
    /// to keep the invocations limited, so you can use more expensive construction in the closure.
    ///
    /// # Panics
    /// If the string is not ASCII.
    pub fn new_cached<T: GodotClass>(init_fn: impl FnOnce() -> String) -> Self {
        // Check if class name exists.
        let type_id = TypeId::of::<T>();
        let mut map = DYNAMIC_INDEX_BY_CLASS_TYPE.lock();

        // Insert into linear vector. Note: this doesn't check for overlaps of TypeId between static and dynamic class names.
        let global_index = *map.entry(type_id).or_insert_with(|| {
            let name = init_fn();
            debug_assert!(name.is_ascii(), "Class name must be ASCII: '{name}'");

            insert_class(StringName::from(name))
        });

        ClassName { global_index }
    }

    #[doc(hidden)]
    pub fn none() -> Self {
        // First element is always the empty string name.
        Self { global_index: 0 }
    }

    #[doc(hidden)]
    pub fn alloc_next(class_name_cstr: &'static CStr) -> Self {
        let global_index = insert_class(StringName::from(class_name_cstr));

        Self { global_index }
    }

    #[doc(hidden)]
    pub fn is_none(&self) -> bool {
        self.global_index == 0
    }
    //
    // /// Returns the class name as a string slice with static storage duration.
    // pub fn as_str(&self) -> &'static str {
    //     // unwrap() safe, checked in constructor
    //     self.c_str.to_str().unwrap()
    // }

    /// Converts the class name to a `GString`.
    pub fn to_gstring(&self) -> GString {
        self.with_string_name(|s| s.into())
    }

    /// Converts the class name to a `StringName`.
    pub fn to_string_name(&self) -> StringName {
        self.with_string_name(|s| s.clone())
    }

    /// The returned pointer is valid indefinitely, as entries are never deleted from the cache.
    /// Since we use `Box<StringName>`, `HashMap` reallocations don't affect the validity of the StringName.
    #[doc(hidden)]
    pub fn string_sys(&self) -> sys::GDExtensionConstStringNamePtr {
        self.with_string_name(|s| s.string_sys())
    }

    // Takes a closure because the mutex guard protects the reference; so the &StringName cannot leave the scope.
    fn with_string_name<R>(&self, func: impl FnOnce(&StringName) -> R) -> R {
        let cached_names = CLASS_NAMES.lock();
        let name = &cached_names[self.global_index as usize];
        func(name)
    }
}

/// Adds a new class name to the cache, returning its index.
fn insert_class(name: StringName) -> u16 {
    let mut names = CLASS_NAMES.lock();
    let index = names
        .len()
        .try_into()
        .expect("Currently limited to 65536 class names");

    names.push(name);
    index
}

impl fmt::Display for ClassName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.with_string_name(|s| s.fmt(f))
    }
}
