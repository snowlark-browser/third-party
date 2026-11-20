/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Generic types for font stuff.

use crate::derives::*;
use crate::parser::{Parse, ParserContext};
use crate::typed_om::{KeywordValue, ToTyped, TypedValue};
use crate::values::animated::ToAnimatedZero;
use crate::{One, Zero};
use byteorder::{BigEndian, ReadBytesExt};
use cssparser::Parser;
use std::fmt::{self, Write};
use std::io::Cursor;
use style_traits::{CssString, CssWriter, ParseError, StyleParseErrorKind, ToCss};
use thin_vec::ThinVec;

/// A trait for values that are labelled with a FontTag (for feature and
/// variation settings).
pub trait TaggedFontValue {
    /// The value's tag.
    fn tag(&self) -> FontTag;
}

/// https://drafts.csswg.org/css-fonts-4/#feature-tag-value
#[derive(
    Clone,
    Debug,
    Deserialize,
    Eq,
    Hash,
    MallocSizeOf,
    PartialEq,
    Serialize,
    SpecifiedValueInfo,
    ToAnimatedValue,
    ToComputedValue,
    ToResolvedValue,
    ToShmem,
)]
pub struct FeatureTagValue<Integer> {
    /// A four-character tag, packed into a u32 (one byte per character).
    pub tag: FontTag,
    /// The actual value.
    pub value: Integer,
}

impl<T> TaggedFontValue for FeatureTagValue<T> {
    fn tag(&self) -> FontTag {
        self.tag
    }
}

impl<Integer> ToCss for FeatureTagValue<Integer>
where
    Integer: One + ToCss + PartialEq,
{
    fn to_css<W>(&self, dest: &mut CssWriter<W>) -> fmt::Result
    where
        W: Write,
    {
        self.tag.to_css(dest)?;
        // Don't serialize the default value.
        if !self.value.is_one() {
            dest.write_char(' ')?;
            self.value.to_css(dest)?;
        }

        Ok(())
    }
}

/// Variation setting for a single feature, see:
///
/// https://drafts.csswg.org/css-fonts-4/#font-variation-settings-def
#[derive(
    Animate,
    Clone,
    ComputeSquaredDistance,
    Debug,
    Deserialize,
    Eq,
    MallocSizeOf,
    PartialEq,
    Serialize,
    SpecifiedValueInfo,
    ToAnimatedValue,
    ToComputedValue,
    ToCss,
    ToResolvedValue,
    ToShmem,
)]
pub struct VariationValue<Number> {
    /// A four-character tag, packed into a u32 (one byte per character).
    #[animation(constant)]
    pub tag: FontTag,
    /// The actual value.
    pub value: Number,
}

impl<T> TaggedFontValue for VariationValue<T> {
    fn tag(&self) -> FontTag {
        self.tag
    }
}

/// A value both for font-variation-settings and font-feature-settings.
#[derive(
    Clone,
    Debug,
    Deserialize,
    Eq,
    Hash,
    MallocSizeOf,
    PartialEq,
    Serialize,
    SpecifiedValueInfo,
    ToAnimatedValue,
    ToCss,
    ToResolvedValue,
    ToShmem,
    ToTyped,
)]
#[css(comma)]
#[typed(todo_derive_fields)]
pub struct FontSettings<T>(#[css(if_empty = "normal", iterable)] pub Box<[T]>);

impl<T> FontSettings<T> {
    /// Default value of font settings as `normal`.
    #[inline]
    pub fn normal() -> Self {
        FontSettings(vec![].into_boxed_slice())
    }
}

impl<T: Parse> Parse for FontSettings<T> {
    /// https://drafts.csswg.org/css-fonts-4/#descdef-font-face-font-feature-settings
    /// https://drafts.csswg.org/css-fonts-4/#font-variation-settings-def
    fn parse(context: &ParserContext, input: &mut Parser) -> Result<Self, ParseError> {
        if input
            .try_parse(|i| i.expect_ident_matching("normal"))
            .is_ok()
        {
            return Ok(Self::normal());
        }

        Ok(FontSettings(
            input
                .parse_comma_separated(|i| T::parse(context, i))?
                .into_boxed_slice(),
        ))
    }
}

/// A font four-character tag, represented as a u32 for convenience.
///
/// See:
///   https://drafts.csswg.org/css-fonts-4/#font-variation-settings-def
///   https://drafts.csswg.org/css-fonts-4/#descdef-font-face-font-feature-settings
///
#[derive(
    Clone,
    Copy,
    Deserialize,
    Eq,
    Hash,
    MallocSizeOf,
    PartialEq,
    Serialize,
    SpecifiedValueInfo,
    ToAnimatedValue,
    ToComputedValue,
    ToResolvedValue,
    ToShmem,
)]
pub struct FontTag(pub u32);

impl fmt::Debug for FontTag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let tag_bytes = self.0.to_be_bytes();

        let mut tuple = f.debug_tuple("FontTag");
        if let Ok(utf8_tag) = str::from_utf8(&tag_bytes) {
            tuple.field(&utf8_tag);
        } else {
            tuple.field(&tag_bytes);
        };
        tuple.finish()
    }
}

impl ToCss for FontTag {
    fn to_css<W>(&self, dest: &mut CssWriter<W>) -> fmt::Result
    where
        W: Write,
    {
        use byteorder::ByteOrder;
        use std::str;

        let mut raw = [0u8; 4];
        BigEndian::write_u32(&mut raw, self.0);
        str::from_utf8(&raw).unwrap_or_default().to_css(dest)
    }
}

impl Parse for FontTag {
    fn parse(_context: &ParserContext, input: &mut Parser) -> Result<Self, ParseError> {
        let tag = input.expect_string()?;

        // allowed strings of length 4 containing chars: <U+20, U+7E>
        if tag.len() != 4 || tag.as_bytes().iter().any(|c| *c < b' ' || *c > b'~') {
            return Err(ParseError::custom(StyleParseErrorKind::UnspecifiedError));
        }

        let mut raw = Cursor::new(tag.as_bytes());
        Ok(FontTag(raw.read_u32::<BigEndian>().unwrap()))
    }
}

/// A generic value for the `font-style` property.
///
/// https://drafts.csswg.org/css-fonts-4/#font-style-prop
#[allow(missing_docs)]
#[derive(
    Animate,
    Clone,
    ComputeSquaredDistance,
    Copy,
    Debug,
    Deserialize,
    Hash,
    MallocSizeOf,
    PartialEq,
    Serialize,
    SpecifiedValueInfo,
    ToAnimatedValue,
    ToAnimatedZero,
    ToResolvedValue,
    ToShmem,
)]
#[value_info(other_values = "normal")]
pub enum FontStyle<Angle> {
    // Note that 'oblique 0deg' represents 'normal', and will serialize as such.
    #[value_info(starts_with_keyword)]
    Oblique(Angle),
    #[animation(error)]
    Italic,
}

impl<Angle: Zero> FontStyle<Angle> {
    /// Return the 'normal' value, which is represented as 'oblique 0deg'.
    pub fn normal() -> Self {
        Self::Oblique(Angle::zero())
    }
}

/// A generic value for the `font-size-adjust` property.
///
/// https://drafts.csswg.org/css-fonts-5/#font-size-adjust-prop
#[allow(missing_docs)]
#[repr(u8)]
#[derive(
    Animate,
    Clone,
    ComputeSquaredDistance,
    Copy,
    Debug,
    Deserialize,
    Hash,
    MallocSizeOf,
    PartialEq,
    Serialize,
    SpecifiedValueInfo,
    ToAnimatedValue,
    ToAnimatedZero,
    ToComputedValue,
    ToResolvedValue,
    ToShmem,
)]
pub enum GenericFontSizeAdjust<Factor> {
    #[animation(error)]
    None,
    #[value_info(starts_with_keyword)]
    ExHeight(Factor),
    #[value_info(starts_with_keyword)]
    CapHeight(Factor),
    #[value_info(starts_with_keyword)]
    ChWidth(Factor),
    #[value_info(starts_with_keyword)]
    IcWidth(Factor),
    #[value_info(starts_with_keyword)]
    IcHeight(Factor),
}

impl<Factor: ToCss> ToCss for GenericFontSizeAdjust<Factor> {
    fn to_css<W>(&self, dest: &mut CssWriter<W>) -> fmt::Result
    where
        W: Write,
    {
        let (prefix, value) = match self {
            Self::None => return dest.write_str("none"),
            Self::ExHeight(v) => ("", v),
            Self::CapHeight(v) => ("cap-height ", v),
            Self::ChWidth(v) => ("ch-width ", v),
            Self::IcWidth(v) => ("ic-width ", v),
            Self::IcHeight(v) => ("ic-height ", v),
        };

        dest.write_str(prefix)?;
        value.to_css(dest)
    }
}

impl<Factor: ToTyped> ToTyped for GenericFontSizeAdjust<Factor> {
    fn to_typed(&self, dest: &mut ThinVec<TypedValue>) -> Result<(), ()> {
        match self {
            Self::None => {
                dest.push(TypedValue::Keyword(KeywordValue(CssString::from("none"))));
                Ok(())
            },
            Self::ExHeight(v) => v.to_typed(dest),
            _ => Err(()),
        }
    }
}

/// A generic value for the `line-height` property.
#[derive(
    Animate,
    Clone,
    ComputeSquaredDistance,
    Copy,
    Debug,
    Deserialize,
    MallocSizeOf,
    PartialEq,
    Serialize,
    SpecifiedValueInfo,
    ToAnimatedValue,
    ToCss,
    ToShmem,
    ToTyped,
)]
#[repr(C, u8)]
pub enum GenericLineHeight<N, L> {
    /// `normal`
    Normal,
    /// `<number>`
    Number(N),
    /// `<length-percentage>`
    Length(L),
}

pub use self::GenericLineHeight as LineHeight;

impl<N, L> ToAnimatedZero for LineHeight<N, L> {
    #[inline]
    fn to_animated_zero(&self) -> Result<Self, ()> {
        Err(())
    }
}

impl<N, L> LineHeight<N, L> {
    /// Returns `normal`.
    #[inline]
    pub fn normal() -> Self {
        LineHeight::Normal
    }

    /// Returns whether the value is `normal`.
    #[inline]
    pub fn is_normal(&self) -> bool {
        matches!(self, Self::Normal)
    }
}

// Snowlark patch: `font-palette` with `palette-mix()` (CSS Fonts 4 §9.1-9.2; SNOWLARK-PATCHES.md 62).

/// One `<palette-identifier> <percentage>?` component of a `palette-mix()`.
#[derive(
    Clone,
    Debug,
    Deserialize,
    MallocSizeOf,
    PartialEq,
    Serialize,
    ToAnimatedValue,
    ToResolvedValue,
    ToShmem,
)]
pub struct GenericPaletteMixItem<Palette, Percentage> {
    /// The palette.
    pub palette: Palette,
    /// Its weight; `None` when it was omitted (and left so next to a calc() sibling).
    pub percentage: Option<Percentage>,
}

/// `palette-mix( <color-interpolation-method>? , [ <font-palette> && <percentage>? ]# )`.
#[derive(
    Clone,
    Debug,
    Deserialize,
    MallocSizeOf,
    PartialEq,
    Serialize,
    ToAnimatedValue,
    ToResolvedValue,
    ToShmem,
)]
pub struct GenericPaletteMix<Palette, Percentage> {
    /// The color space (and hue method) the palettes' colors mix in.
    pub interpolation: crate::color::mix::ColorInterpolationMethod,
    /// The palettes and their weights.
    pub items: crate::OwnedSlice<GenericPaletteMixItem<Palette, Percentage>>,
}

impl<Palette: ToCss, Percentage: ToCss + crate::values::specified::percentage::ToPercentage> ToCss
    for GenericPaletteMix<Palette, Percentage>
{
    /// Serialized as `color-mix()` is (CSS Color 5 §2.5): the default `in oklab` omitted, and every
    /// percentage omitted iff all are known (not calc) and equal to 100%/N.
    fn to_css<W>(&self, dest: &mut CssWriter<W>) -> fmt::Result
    where
        W: Write,
    {
        dest.write_str("palette-mix(")?;
        if !self.interpolation.is_default() {
            self.interpolation.to_css(dest)?;
            dest.write_str(", ")?;
        }
        let uniform = 1.0 / self.items.len() as f32;
        let omit_all = self.items.iter().all(|item| {
            item.percentage
                .as_ref()
                .is_some_and(|p| !p.is_calc() && p.to_percentage() == Some(uniform))
        });
        for (index, item) in self.items.iter().enumerate() {
            if index != 0 {
                dest.write_str(", ")?;
            }
            item.palette.to_css(dest)?;
            if omit_all {
                continue;
            }
            if let Some(percentage) = item.percentage.as_ref() {
                dest.write_char(' ')?;
                percentage.to_css(dest)?;
            }
        }
        dest.write_char(')')
    }
}

/// A `font-palette` value: `normal | light | dark | <palette-identifier> | <palette-mix()>`.
#[derive(
    Clone,
    Debug,
    Deserialize,
    MallocSizeOf,
    PartialEq,
    Serialize,
    ToAnimatedValue,
    ToResolvedValue,
    ToShmem,
)]
pub enum GenericFontPalette<Percentage> {
    /// `normal`, `light`, `dark` or a `<dashed-ident>`.
    Named(crate::Atom),
    /// `palette-mix()`.
    Mix(Box<GenericPaletteMix<GenericFontPalette<Percentage>, Percentage>>),
}

impl<Percentage> GenericFontPalette<Percentage> {
    /// `normal`
    pub fn normal() -> Self {
        Self::Named(atom!("normal"))
    }
    /// `light`
    pub fn light() -> Self {
        Self::Named(atom!("light"))
    }
    /// `dark`
    pub fn dark() -> Self {
        Self::Named(atom!("dark"))
    }
}

impl<Percentage: ToCss + crate::values::specified::percentage::ToPercentage> ToCss
    for GenericFontPalette<Percentage>
{
    fn to_css<W>(&self, dest: &mut CssWriter<W>) -> fmt::Result
    where
        W: Write,
    {
        match self {
            Self::Named(name) => crate::values::serialize_atom_identifier(name, dest),
            Self::Mix(mix) => mix.to_css(dest),
        }
    }
}

impl<Percentage> ToTyped for GenericFontPalette<Percentage> {
    fn to_typed(&self, dest: &mut ThinVec<TypedValue>) -> Result<(), ()> {
        match self {
            Self::Named(name) => {
                dest.push(TypedValue::Keyword(KeywordValue(CssString::from(
                    name.to_string(),
                ))));
                Ok(())
            },
            Self::Mix(..) => Err(()),
        }
    }
}

impl<Percentage> style_traits::SpecifiedValueInfo for GenericFontPalette<Percentage> {
    fn collect_completion_keywords(f: style_traits::KeywordsCollectFn) {
        f(&["normal", "light", "dark", "palette-mix"]);
    }
}
