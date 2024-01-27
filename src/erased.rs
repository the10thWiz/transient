/*!
Defines safe wrappers for erasing and restoring transient types.

This module defines the `Erased`, `ErasedRef`, and `ErasedMut` structs for
wrapping owned values, shared references, and mutable references, respectively,
that have been transmuted to `'static` and cast to `dyn Any`. While artificially
extending lifetimes is typically very unsafe, each wrapper struct provides a
safe interface to the falsely-`'static` value it wraps by restricting safe
access to it until the *true* lifetime has been restored.

In order to enforce this restriction, the safe public API does not expose the
wrapped value directly, which in principle could be downcast and cloned to
obtain a transient value with an unbounded lifetime. However, this restriction
is lifted when the *true* lifetime is static, since use-after-free is no longer
a concern.
*/

use std::{any::{Any, TypeId}, marker::PhantomData};
use super::MakeStatic;


/// Safely wraps a potentially non-`'static` value that has been transmuted
/// to `'static` and cast to `Box<dyn Any>`.
///
/// This struct provides a safe interface for erasing/restoring such a type by
/// restricting access to the falsely-`'static` object and ensuring that it cannot
/// be used after its *true* lifetime `'src`. To enforce this condition, the safe 
/// public API does not expose the wrapped value directly, which in principle could 
/// be downcast and cloned to obtain a transient value with an unbounded lifetime.
#[derive(Debug)]
pub struct Erased<'src>(
    Box<dyn Any>, // DO NOT EXPOSE!
    PhantomData<&'src ()>
);

impl<'src> Erased<'src> {

    /// Erase and wrap a transient value with lifetime `'src`.
    pub fn new<T: MakeStatic<'src>>(value: T) -> Self {
        let boxed = Box::new(value);
        let extended: Box<T::Static> = unsafe {boxed.make_static_owned()};
        Self(extended, PhantomData)
    }
    /// Erase and wrap a boxed transient value with lifetime `'src`.
    pub fn from_boxed<T: MakeStatic<'src>>(boxed: Box<T>) -> Self {
        let extended: Box<T::Static> = unsafe {boxed.make_static_owned()};
        Self(extended, PhantomData)
    }

    /// Safely restore the type and lifetime of the wrapped value.
    ///
    /// If the conversion fails, `self` is rebuilt and returned in the `Err`
    /// variant so that the caller can regain ownership. To return the value
    /// without unboxing, call [`restore_box`][Erased::restore_box] instead.
    pub fn restore<T: MakeStatic<'src>>(self) -> Result<T, Self> {
        Ok(*self.restore_box::<T>()?)
    }

    /// Safely restore the type and lifetime of the wrapped value and return 
    /// it in a `Box`.
    ///
    /// If the conversion fails, `self` is rebuilt and returned in the `Err`
    /// variant so that the caller can regain ownership. To return the value
    /// unboxed, call [`restore`][Erased::restore] instead.
    pub fn restore_box<T: MakeStatic<'src>>(self) -> Result<Box<T>, Self> {
        let restored = self.0.downcast::<T::Static>()
            .map_err(|inner| Erased(inner, PhantomData))?;
        // the lifetime of the pointed-to value must have been 'src
        // for `self` to be created from safe code
        let shortened: Box<T> = unsafe {T::from_static_owned(restored)};
        Ok(shortened)
    }

    /// Get the `TypeId` of the wrapped value (see [`Any::type_id`]).
    pub fn type_id(&self) -> TypeId {
        (&*self.0).type_id()
    }

    /// Check whether the wrapped value has the given type (see
    /// [`<dyn Any>::is`](https://doc.rust-lang.org/std/any/trait.Any.html#method.is)).
    ///
    /// Note that this comparison does not take the lifetime of `T` into account; an
    /// erased value with original type `&'src str` will return `true` when compared to
    /// any `&'_ str` including `&'static str`, even though they would typically be
    /// classified as distinct types.
    pub fn is<T: MakeStatic<'src>>(&self) -> bool {
        (&*self.0).is::<T::Static>()
    }
}


/// Safely wraps a shared reference to a potentially non-`'static` value that
/// has been transmuted to `'static` and cast to `&dyn Any`.
///
/// This struct provides a safe interface for erasing/restoring such a type by
/// restricting access to the falsely-`'static` object and ensuring that it cannot
/// be used after its *true* lifetime `'src`. To enforce this condition, the safe
/// public API does not expose the wrapped value directly, which in principle
/// could be downcast and cloned to obtain a transient value with an unbounded
/// lifetime.
#[derive(Debug, Clone, Copy)]
pub struct ErasedRef<'borrow, 'src: 'borrow>(
    &'borrow dyn Any, // DO NOT EXPOSE!
    PhantomData<&'src ()>,
);

impl<'borrow, 'src: 'borrow> ErasedRef<'borrow, 'src> {

    /// Erase and wrap a shared reference to a value with lifetime `'src` that
    /// has been borrowed with lifetime `'borrow`.
    pub fn new<T: MakeStatic<'src>>(value: &'borrow T) -> Self {
        Self(unsafe {value.make_static_ref()}, PhantomData)
    }

    /// Safely restore the original type `T` and lifetime `'src` of the pointee.
    ///
    /// If the conversion fails, `self` is rebuilt and returned in the `Err`
    /// variant so that the caller can regain ownership.
    pub fn restore<T: MakeStatic<'src>>(self) -> Result<&'borrow T, Self> {
        let restored = self.0.downcast_ref::<T::Static>().ok_or(self)?;
        // the true lifetime must have been 'src for `self` to be created from safe code
        let shortened = unsafe {T::from_static_ref(restored)};
        Ok(shortened)
    }

    /// Get the `TypeId` of the wrapped value (see [`Any::type_id`]).
    pub fn type_id(&self) -> TypeId {
        self.0.type_id()
    }

    /// Check whether the pointee of the wrapped reference has the given type (see
    /// [`<dyn Any>::is`](https://doc.rust-lang.org/std/any/trait.Any.html#method.is)).
    ///
    /// Note that this comparison does not take the lifetime of `T` into account; an
    /// erased value with original type `&'src str` will return `true` when compared to
    /// any `&'_ str` including `&'static str`, even though they would typically be
    /// classified as distinct types.
    pub fn is<T: MakeStatic<'src>>(&self) -> bool {
        self.0.is::<T::Static>()
    }
}

/// Safely wraps a mutable reference to a potentially non-`'static` value that
/// has been transmuted to `'static` and cast to `&mut dyn Any`.
///
/// This struct provides a safe interface for erasing/restoring such a type by
/// restricting access to the falsely-`'static` object and ensuring that it cannot
/// be used after its *true* lifetime `'src`. To enforce this condition, the safe public
/// API does not expose the wrapped value directly, which in principle could be
/// downcast and cloned to obtain a transient value with an unbounded lifetime.
#[derive(Debug)]
pub struct ErasedMut<'borrow, 'src: 'borrow>(
    &'borrow mut dyn Any, // DO NOT EXPOSE!
    PhantomData<&'src ()>,
);

impl<'borrow, 'src: 'borrow> ErasedMut<'borrow, 'src> {

    /// Erase and wrap a mutable reference to a value with lifetime `'src` that
    /// has been borrowed with lifetime `'borrow`.
    pub fn new<T: MakeStatic<'src>>(value: &'borrow mut T) -> Self {
        Self(unsafe {value.make_static_mut()}, PhantomData)
    }

    /// Safely restore the original type `T` and lifetime `'src` of the pointee.
    ///
    /// If the conversion fails, `self` is rebuilt and returned in the `Err`
    /// variant so that the caller can regain ownership.
    pub fn restore<T: MakeStatic<'src>>(self) -> Result<&'borrow mut T, Self> {
        if self.is::<T>() {
            let restored = self.0.downcast_mut::<T::Static>().unwrap();
            // the true lifetime must have been 'src for `self` to be created from safe code
            Ok(unsafe {T::from_static_mut(restored)})
        } else {
            Err(self)
        }
    }

    /// Get the `TypeId` of the wrapped value (see [`Any::type_id`])
    pub fn type_id(&self) -> TypeId {
        (&*self.0).type_id()
    }

    /// Check whether the pointee of the wrapped reference has the given type (see
    /// [`<dyn Any>::is`](https://doc.rust-lang.org/std/any/trait.Any.html#method.is)).
    ///
    /// Note that this comparison does not take the lifetime of `T` into account; an
    /// erased value with original type `&'src str` will return `true` when compared to
    /// any `&'_ str` including `&'static str`, even though they would typically be
    /// classified as distinct types.
    pub fn is<T: MakeStatic<'src>>(&self) -> bool {
        (&*self.0).is::<T::Static>()
    }
}

// === METHODS FOR ACCESSING THE WRAPPED VALUE === //

/// These methods are only implemented when `'src: 'static`, since access to
/// a transient value with an artificially extended lifetime would be unsafe.
impl Erased<'static> {
    /// Get a shared reference to the wrapped `dyn Any`; only implemented
    /// when the original value was `'static`.
    pub fn inner(&self) -> &dyn Any {
        &*self.0
    }
    /// Get a mutable reference to the wrapped `dyn Any`; only implemented
    /// when the original value was `'static`.
    pub fn inner_mut(&mut self) -> &mut dyn Any {
        &mut *self.0
    }
    /// Move out of the wrapper and return the wrapped `Box<dyn Any>`; only
    /// implemented when the original value was `'static`.
    pub fn into_inner(self) -> Box<dyn Any> {
        self.0
    }
}

/// These methods are only implemented when `'src: 'static`, since access to
/// a transient value with an artificially extended lifetime would be unsafe.
impl<'borrow> ErasedRef<'borrow, 'static> {
    /// Get a shared reference to the wrapped `dyn Any`; only implemented
    /// when the original value was `'static`.
    pub fn inner(&self) -> &dyn Any {
        &*self.0
    }
}

/// These methods are only implemented when `'src: 'static`, since access to
/// a transient value with an artificially extended lifetime would be unsafe.
impl<'borrow> ErasedMut<'borrow, 'static> {
    /// Get a shared reference to the wrapped `dyn Any`; only implemented
    /// when the original value was `'static`.
    pub fn inner(&self) -> &dyn Any {
        &*self.0
    }
    /// Get a mutable reference to the wrapped `dyn Any`; only implemented
    /// when the original value was `'static`.
    pub fn inner_mut(&mut self) -> &mut dyn Any {
        &mut *self.0
    }
}

// === CONVERSION METHODS === //

/// Since a `dyn Any` created from safe code has an implicit `'static` bound,
/// we can place it directly into the wrapper with `'src: 'static`. However,
/// note that the `Erased::restore` method will restore the original value
/// that was cast to `dyn Any`, not the erased `dyn Any` itself.
impl From<Box<dyn Any>> for Erased<'static> {
    fn from(value: Box<dyn Any>) -> Self {
        Erased(value, PhantomData)
    }
}

/// Since a `dyn Any` created from safe code has an implicit `'static` bound,
/// we can place it directly into the wrapper with `'src: 'static`. However,
/// note that the `ErasedRef::restore` method will restore the original value
/// that was cast to `dyn Any`, not the erased `dyn Any` itself.
impl<'borrow> From<&'borrow dyn Any> for ErasedRef<'borrow, 'static> {
    fn from(value: &'borrow dyn Any) -> Self {
        ErasedRef(value, PhantomData)
    }
}

/// Since a `dyn Any` created from safe code has an implicit `'static` bound,
/// we can place it directly into the wrapper with `'src: 'static`. However,
/// note that the `ErasedRef::restore` method will restore the original value
/// that was cast to `dyn Any`, not the erased `dyn Any` itself.
impl<'borrow> From<&'borrow mut dyn Any> for ErasedRef<'borrow, 'static> {
    fn from(value: &'borrow mut dyn Any) -> Self {
        ErasedRef(&*value, PhantomData)
    }
}

/// Since a `dyn Any` created from safe code has an implicit `'static` bound,
/// we can place it directly into the wrapper with `'src: 'static`. However,
/// note that the `ErasedMut::restore` method will restore the original value
/// that was cast to `dyn Any`, not the erased `dyn Any` itself.
impl<'borrow> From<&'borrow mut dyn Any> for ErasedMut<'borrow, 'static> {
    fn from(value: &'borrow mut dyn Any) -> Self {
        ErasedMut(value, PhantomData)
    }
}
