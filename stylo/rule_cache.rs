/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! A cache from rule node to computed values, in order to cache reset
//! properties.

use crate::computed_value_flags::ComputedValueFlags;
use crate::context::CascadeInputs;
use crate::logical_geometry::WritingMode;
use crate::properties::{ComputedValues, StyleBuilder};
use crate::rule_tree::{RuleCascadeFlags, StrongRuleNode};
use crate::selector_parser::PseudoElement;
use crate::shared_lock::StylesheetGuards;
use crate::values::computed::{Context, NonNegativeLength};
use crate::values::specified::color::ColorSchemeFlags;
use rustc_hash::FxHashMap;
use servo_arc::Arc;
use smallvec::SmallVec;

/// The conditions for caching and matching a style in the rule cache.
#[derive(Clone, Debug, Default)]
pub struct RuleCacheConditions {
    uncacheable: bool,
    font_size: Option<NonNegativeLength>,
    line_height: Option<NonNegativeLength>,
    writing_mode: Option<WritingMode>,
    color_scheme: Option<ColorSchemeFlags>,
}

impl RuleCacheConditions {
    /// Sets the style as depending in the font-size value.
    pub fn set_font_size_dependency(&mut self, font_size: NonNegativeLength) {
        debug_assert!(self.font_size.is_none_or(|f| f == font_size));
        self.font_size = Some(font_size);
    }

    /// Sets the style as depending in the line-height value.
    pub fn set_line_height_dependency(&mut self, line_height: NonNegativeLength) {
        debug_assert!(self.line_height.is_none_or(|l| l == line_height));
        self.line_height = Some(line_height);
    }

    /// Sets the style as depending in the color-scheme property value.
    pub fn set_color_scheme_dependency(&mut self, color_scheme: ColorSchemeFlags) {
        debug_assert!(self.color_scheme.is_none_or(|cs| cs == color_scheme));
        self.color_scheme = Some(color_scheme);
    }

    /// Sets the style as uncacheable.
    pub fn set_uncacheable(&mut self) {
        self.uncacheable = true;
    }

    /// Sets the style as depending in the writing-mode value `writing_mode`.
    pub fn set_writing_mode_dependency(&mut self, writing_mode: WritingMode) {
        debug_assert!(self.writing_mode.is_none_or(|wm| wm == writing_mode));
        self.writing_mode = Some(writing_mode);
    }

    /// Returns whether the current style's reset properties are cacheable.
    fn cacheable(&self) -> bool {
        !self.uncacheable
    }
}

#[derive(Debug)]
struct CachedConditions {
    font_size: Option<NonNegativeLength>,
    line_height: Option<NonNegativeLength>,
    color_scheme: Option<ColorSchemeFlags>,
    writing_mode: Option<WritingMode>,
}

impl CachedConditions {
    /// Returns whether `style` matches the conditions.
    fn matches(&self, cached_style: &ComputedValues, style: &StyleBuilder) -> bool {
        if cached_style.effective_zoom != style.effective_zoom {
            return false;
        }

        if cached_style
            .flags
            .intersects(ComputedValueFlags::IS_IN_APPEARANCE_BASE_SUBTREE)
            != style
                .flags()
                .intersects(ComputedValueFlags::IS_IN_APPEARANCE_BASE_SUBTREE)
        {
            return false;
        }

        if let Some(fs) = self.font_size {
            if style.get_font().clone_font_size().computed_size != fs {
                return false;
            }
        }

        if let Some(lh) = self.line_height {
            let new_line_height =
                style
                    .device
                    .calc_line_height(style.get_font(), style.writing_mode, None);
            if new_line_height != lh {
                return false;
            }
        }

        if self
            .color_scheme
            .is_some_and(|cs| style.get_inherited_ui().color_scheme_bits() != cs)
        {
            return false;
        }

        if self.writing_mode.is_some_and(|wm| style.writing_mode != wm) {
            return false;
        }

        true
    }
}

/// Snowlark patch 66: what a computed style read from the device at the time it was kept by a
/// [`MatchedPropertiesCache`] (`Device::snowlark_root_and_viewport`).
#[cfg(feature = "servo")]
type DeviceState = ([u32; 6], [i32; 2], crate::color::AbsoluteColor);

#[cfg(feature = "servo")]
use crate::properties::longhands::display::computed_value::T as Display;

#[cfg(feature = "servo")]
type ElementLocalName =
    <crate::selector_parser::SelectorImpl as selectors::parser::SelectorImpl>::BorrowedLocalName;
#[cfg(feature = "servo")]
type ElementNamespace =
    <crate::selector_parser::SelectorImpl as selectors::parser::SelectorImpl>::BorrowedNamespaceUrl;

/// Snowlark patch 66: whole computed styles kept across restyles, by the rules matched and what
/// else the cascade read: the element's name, its parent's inherited values and its layout
/// parent's display and writing mode. An element whose rules, name and parent all equal a kept
/// one's gets that element's style (the same `Arc`) without a cascade.
///
/// Rule nodes stand for their declarations: a style attribute or presentational hints that change
/// get a new declaration block, and so a new rule node. What can change a kept style without
/// changing its rule node (new or edited style sheets, the device's media features, the color
/// schemes) must `clear` the cache before the next traversal; a change of the root element's font
/// size, line height or font metrics, the viewport size or the body text color empties it on the
/// next `insert`, and `find` misses until then. A style that read anything else about its element
/// or its place in the tree (container queries and units, sibling counts, `attr()`, font metrics,
/// a reset property inherited from its parent, a link, the root) is not kept.
#[cfg(feature = "servo")]
#[derive(Default)]
pub struct MatchedPropertiesCache {
    map: FxHashMap<StrongRuleNode, SmallVec<[MatchedProperties; 2]>>,
    device: Option<DeviceState>,
}

/// One entry of a [`MatchedPropertiesCache`].
#[cfg(feature = "servo")]
pub struct MatchedProperties {
    local_name: ElementLocalName,
    namespace: ElementNamespace,
    input_flags: ComputedValueFlags,
    /// The parent's style (only its inherited values are compared).
    parent: crate::properties::ComputedValuesInner,
    /// The layout parent's display and writing mode, and whether it is the parent.
    layout_parent: (Display, WritingMode, bool),
    style: Arc<ComputedValues>,
}

#[cfg(feature = "servo")]
impl MatchedPropertiesCache {
    /// Rule nodes kept at most (each with up to 4 styles); a full cache is emptied.
    const MAX: usize = 8192;

    /// What a style must not depend on to be kept.
    const UNCACHEABLE: ComputedValueFlags = ComputedValueFlags::INHERITS_RESET_STYLE
        .union(ComputedValueFlags::IS_ROOT_ELEMENT_STYLE)
        .union(ComputedValueFlags::IS_RELEVANT_LINK_VISITED)
        .union(ComputedValueFlags::IS_IN_FIRST_LINE_SUBTREE)
        .union(ComputedValueFlags::DEPENDS_ON_SELF_FONT_METRICS)
        .union(ComputedValueFlags::DEPENDS_ON_INHERITED_FONT_METRICS)
        .union(ComputedValueFlags::USES_VIEWPORT_UNITS_ON_CONTAINER_QUERIES)
        .union(ComputedValueFlags::USES_CONTAINER_UNITS)
        .union(ComputedValueFlags::CONSIDERED_NONTRIVIAL_SCOPED_STYLE)
        .union(ComputedValueFlags::DISPLAY_CONTENTS_IN_ITEM_CONTAINER)
        .union(ComputedValueFlags::DEPENDS_ON_CONTAINER_STYLE_QUERY)
        .union(ComputedValueFlags::USES_FONT_OR_WM_RELATIVE_UNITS_ON_CONTAINER_QUERIES)
        .union(ComputedValueFlags::USES_SIBLING_COUNT)
        .union(ComputedValueFlags::USES_SIBLING_INDEX);

    /// Whether `rules` hold animation or transition declarations (which stand for values computed
    /// at a moment, and may be the same block after what they were computed from changed). They
    /// sit above every normal author level: the walk stops at the first normal level.
    pub fn animated(rules: &StrongRuleNode) -> bool {
        let mut node = Some(rules);
        while let Some(n) = node {
            let level = n.cascade_priority().cascade_level();
            if level.is_animation() {
                return true;
            }
            if !level.is_important() {
                return false;
            }
            node = n.parent();
        }
        false
    }

    /// Whether the cache keeps nothing.
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// Empties the cache.
    pub fn clear(&mut self) {
        self.map.clear();
        self.device = None;
    }

    fn layout_parent_key(
        parent: &ComputedValues,
        layout_parent: &ComputedValues,
    ) -> (Display, WritingMode, bool) {
        (
            layout_parent.get_box().clone_display(),
            layout_parent.writing_mode,
            std::ptr::eq(parent, layout_parent),
        )
    }

    /// The kept style for an element named `local_name` in `namespace` whose rules are `rules`,
    /// under `parent` and `layout_parent`.
    pub fn find(
        &self,
        device: &crate::device::Device,
        rules: &StrongRuleNode,
        local_name: &ElementLocalName,
        namespace: &ElementNamespace,
        input_flags: ComputedValueFlags,
        parent: &ComputedValues,
        layout_parent: &ComputedValues,
    ) -> Option<&Arc<ComputedValues>> {
        let entries = self.map.get(rules)?;
        if self.device.as_ref() != Some(&device.snowlark_root_and_viewport()) {
            return None;
        }
        let layout = Self::layout_parent_key(parent, layout_parent);
        entries
            .iter()
            .find(|e| {
                e.local_name == *local_name
                    && e.namespace == *namespace
                    && e.input_flags == input_flags
                    && e.layout_parent == layout
                    && e.parent.snowlark_inherits_equal(parent)
            })
            .map(|e| &e.style)
    }

    /// Keeps `style`, the style of an element named `local_name` in `namespace` under `parent` and
    /// `layout_parent` (rules: its own), when nothing but those decided it.
    pub fn insert(
        &mut self,
        device: &crate::device::Device,
        local_name: &ElementLocalName,
        namespace: &ElementNamespace,
        input_flags: ComputedValueFlags,
        parent: &ComputedValues,
        layout_parent: &ComputedValues,
        style: &Arc<ComputedValues>,
    ) {
        let Some(rules) = style.rules.as_ref() else {
            return;
        };
        if style.flags.intersects(Self::UNCACHEABLE)
            || style.attribute_references.is_some()
            || style.visited_style().is_some()
        {
            return;
        }
        let state = device.snowlark_root_and_viewport();
        if self.device.as_ref() != Some(&state) || self.map.len() >= Self::MAX {
            self.map.clear();
            self.device = Some(state);
        }
        let entries = self.map.entry(rules.clone()).or_default();
        if entries.len() >= 4 {
            entries.remove(0);
        }
        entries.push(MatchedProperties {
            local_name: local_name.clone(),
            namespace: namespace.clone(),
            input_flags,
            parent: (**parent).clone(),
            layout_parent: Self::layout_parent_key(parent, layout_parent),
            style: style.clone(),
        });
    }

    /// Moves the styles `other` keeps into this cache (`other` is left empty).
    pub fn take_from(&mut self, other: &mut MatchedPropertiesCache) {
        if other.map.is_empty() {
            return;
        }
        if self.device != other.device || self.map.len() + other.map.len() > Self::MAX {
            self.map.clear();
            self.device = other.device.clone();
        }
        if self.map.is_empty() {
            std::mem::swap(&mut self.map, &mut other.map);
            return;
        }
        for (rules, entries) in other.map.drain() {
            let mine = self.map.entry(rules).or_default();
            for e in entries {
                if mine.len() >= 4 {
                    mine.remove(0);
                }
                mine.push(e);
            }
        }
    }
}

/// A TLS cache from rules matched to computed values.
pub struct RuleCache {
    // FIXME(emilio): Consider using LRUCache or something like that?
    map: FxHashMap<StrongRuleNode, SmallVec<[(CachedConditions, Arc<ComputedValues>); 1]>>,
}

impl Default for RuleCache {
    fn default() -> Self {
        Self::new()
    }
}

impl RuleCache {
    /// Creates an empty `RuleCache`.
    pub fn new() -> Self {
        Self {
            map: FxHashMap::default(),
        }
    }

    /// Walk the rule tree and return a rule node for using as the key
    /// for rule cache.
    ///
    /// It currently skips animation / style attribute / preshint rules when they don't contain any
    /// declaration of a reset property. We don't skip other levels because walking the whole
    /// parent chain can be expensive.
    ///
    /// TODO(emilio): Measure this, this was not super-well measured for performance (this was done
    /// for memory in bug 1427681)... Walking the rule tree might be worth it if we hit the cache
    /// enough?
    fn get_rule_node_for_cache<'r>(
        guards: &StylesheetGuards,
        mut rule_node: Option<&'r StrongRuleNode>,
    ) -> Option<&'r StrongRuleNode> {
        use crate::rule_tree::CascadeOrigin;
        while let Some(node) = rule_node {
            let priority = node.cascade_priority();
            let cascade_level = priority.cascade_level();
            let should_try_to_skip = cascade_level.is_animation()
                || cascade_level.origin() == CascadeOrigin::PresHints
                || priority.layer_order().is_style_attribute_layer();
            if !should_try_to_skip {
                break;
            }
            if let Some(source) = node.style_source() {
                let decls = source.get().read_with(cascade_level.guard(guards));
                if decls.contains_any_reset() {
                    break;
                }
            }
            rule_node = node.parent();
        }
        rule_node
    }

    /// Finds a node in the properties matched cache.
    ///
    /// This needs to receive a `StyleBuilder` with the `early` properties
    /// already applied.
    pub fn find(&self, guards: &StylesheetGuards, context: &Context) -> Option<&ComputedValues> {
        // A pseudo-element with property restrictions can result in different
        // computed values if it's also used for a non-pseudo.
        if context
            .builder
            .pseudo
            .and_then(|p| p.property_restriction())
            .is_some()
        {
            return None;
        }

        if context
            .included_cascade_flags
            .contains(RuleCascadeFlags::STARTING_STYLE)
        {
            // We don't want to cache nor include starting-style rules.
            return None;
        }

        let rules = context.builder.rules.as_ref();
        let rules = Self::get_rule_node_for_cache(guards, rules)?;
        let cached_values = self.map.get(rules)?;

        for (conditions, values) in cached_values.iter() {
            if conditions.matches(values, &context.builder) {
                debug!("Using cached reset style with conditions {:?}", conditions);
                return Some(&**values);
            }
        }
        None
    }

    /// Inserts a node into the rules cache if possible.
    ///
    /// Returns whether the style was inserted into the cache.
    pub fn insert_if_possible(
        &mut self,
        guards: &StylesheetGuards,
        style: &Arc<ComputedValues>,
        pseudo: Option<&PseudoElement>,
        inputs: &CascadeInputs,
        conditions: &RuleCacheConditions,
    ) -> bool {
        if !conditions.cacheable() {
            return false;
        }

        // A pseudo-element with property restrictions can result in different computed values if
        // it's also used for a non-pseudo.
        // TODO: we could consider inserting them and just checking the builder like we do for zoom.
        if pseudo.and_then(|p| p.property_restriction()).is_some() {
            return false;
        }

        // Don't insert @starting-style styles in the cache, for the same reason.
        if inputs
            .included_cascade_flags
            .contains(RuleCascadeFlags::STARTING_STYLE)
        {
            return false;
        }

        let rules = style.rules.as_ref();
        let rules = match Self::get_rule_node_for_cache(guards, rules) {
            Some(r) => r.clone(),
            None => return false,
        };

        debug!(
            "Inserting cached reset style with conditions {:?}",
            conditions
        );
        let cached_conditions = CachedConditions {
            writing_mode: conditions.writing_mode,
            font_size: conditions.font_size,
            line_height: conditions.line_height,
            color_scheme: conditions.color_scheme,
        };
        self.map
            .entry(rules)
            .or_default()
            .push((cached_conditions, style.clone()));
        true
    }
}
