# stylo 0.22.0, patched for Snowlark

This is stylo 0.22.0 from crates.io (the first commit that adds this directory is the unmodified release),
used through `[patch.crates-io]` in the workspace `Cargo.toml`. Every change is marked `Snowlark patch` in
the source; `git log -p vendor/stylo` shows the diff.

1. **`@container` in the servo build** (`stylesheets/rule_parser.rs`): upstream parses `@container` only
   with the `gecko` feature, although the servo build has the container query evaluation, the
   `container-type`/`container-name` properties and the `TElement::query_container_size` hook. The rule
   now also parses when the `layout.container-queries.enabled` preference is on (snowlark-style's
   `prefs::init` turns it on).

2. **Used color scheme** (CSS Color Adjust 1 §2): `device/servo.rs` gets the page color scheme (the
   HTML `<meta name=color-scheme>`, set by snowlark-style through `Device::set_page_color_scheme`) and
   `used_color_scheme_is_dark`, which `is_dark_color_scheme` (system colors, `light-dark()`) now uses:
   `normal` takes the page color scheme and, without one, the UA default light (upstream used the
   preferred scheme); one listed scheme is used; both listed follow the preferred one. Every built
   style records the decision in `ComputedValueFlags::USED_DARK_COLOR_SCHEME`
   (`properties.mako.rs` `StyleBuilder::build`, `computed_value_flags.rs`) for what the embedder
   paints itself (scroll bars, the canvas). The dark `Canvas`, `CanvasText`, `Field` and `FieldText`
   take Chrome's values (#121212, white, #3B3B3B, white).
3. **`color: initial` is `CanvasText`** (CSS Color 4 §3.1) for the element's color scheme
   (`properties/helpers.mako.rs`), not plain black.
4. **`pointer-events` takes SVG 2's values** (SVG 2 §16.6: `bounding-box`, `visiblePainted`,
   `visibleFill`, `visibleStroke`, `visible`, `painted`, `fill`, `stroke`, `all`) in the servo build
   (`values/specified/ui.rs`); upstream parsed them for gecko only, so `pointer-events: all` was an
   invalid declaration and an element under a `pointer-events: none` ancestor stayed `none`.
5. **Container query length units in the servo build** (`values/specified/length.rs`): upstream parses
   `cqw`/`cqh`/`cqi`/`cqb`/`cqmin`/`cqmax` only with the `gecko` feature, so a declaration using them
   (directly or through `var()`, e.g. `max-height: min(540px, 100cqw)`) was invalid (at computed-value
   time) and fell back to its initial value. They now parse under the same
   `layout.container-queries.enabled` preference as `@container`; the servo build already resolves them
   against the query container (`Context::get_container_size_query`, the small viewport as fallback).
6. **`CalcLengthPercentage::node`** (`values/computed/length_percentage.rs`): a public accessor for the
   computed `calc()` tree, so snowlark-render maps `min()`/`max()`/`clamp()` mixing lengths and
   percentages to layout's bounded linear form instead of approximating them as a line.
7. **`contain-intrinsic-*` in the servo build** (`properties/longhands.toml`, `properties/shorthands.toml`):
   `contain-intrinsic-width`/`-height`/`-inline-size`/`-block-size` and the `contain-intrinsic-size`
   shorthand were gecko-only; they now parse and compute in the servo build behind
   `layout.unimplemented` (CSS Sizing 4 §5.1, the size a size-contained box takes in place of its
   content).

8. **Script animations in the cascade** (Web Animations 1 §5.4): `TElement::process_script_animations`
   (`dom.rs`, default `false`) is called from the servo `process_animations` (`matching.rs`) after the
   CSS animations are updated; when it reports that an element's script-animation keyframes were
   (re)computed, the element is recascaded, and that recascade takes the element's own
   `animation_rule`/`transition_rule` (which add script animations to the CSS ones) instead of
   `DocumentAnimationSet::get_all_declarations`. A transition the restyle would start for a property
   a script animation animates (`TElement::animated_by_script`) is dropped: the animation's value
   covers both the before-change and the after-change style. `Transition::created_at` keeps the
   start time a transition was created with (`servo/animation.rs`): it identifies the transition
   after script moved its `start_time` through its `CSSTransition`.
9. **`text-box-trim`, `text-box-edge` and the `text-box` shorthand in the servo build**
   (`properties/longhands.toml`, `properties/shorthands.toml`, `properties/shorthands.rs`): upstream
   declares them for the gecko engine only, though their value types are engine-neutral. CSS Inline 3
   §6; snowlark-layout implements them.
10. **`background-attachment: local` in the servo build** (`properties/longhands.toml`): upstream parses
   `local` for gecko only; CSS Backgrounds 3 §3.5 defines it and snowlark-layout paints it.
11. **Legacy line clamping in the servo build** (CSS Overflow 4 §5.2): `display: -webkit-box` and
    `-webkit-inline-box` and the `Display::FlowRoot` constant are no longer Gecko-only
    (`values/specified/box.rs`); the `line-clamp` longhand exists in the servo build behind
    `layout.css.line-clamp.enabled` and the `-webkit-line-clamp` legacy shorthand always
    (`properties/longhands.toml`, `shorthands.toml`); `-webkit-box-orient` is a servo-only longhand of
    the box struct (Gecko has it as an alias in its XUL struct); and `adjust_for_webkit_line_clamp`
    (`style_adjuster.rs`) runs in both builds, turning a vertical `-webkit-box` with a clamp into
    `flow-root` (or `inline-block`).
12. **`SharedRwLock::same_lock`** (`shared_lock.rs`): tells whether two locks are one, so a constructed
    `CSSStyleSheet` is adopted only into the cascade of the document whose lock it was parsed under.
13. **Unclamped `calc()` resolution** (`values/computed/length_percentage.rs`):
   `LengthPercentage::resolve_unclamped`. A computed `calc()` clamps its result to the property's
   range, so snowlark-render, which takes a linear `calc()` apart into a length and a percentage by
   resolving it at two bases, read `calc(50% - 10px)` in `width` as 49% (the value at base 0 clamped
   to zero).
14. **Pseudo-elements after `::slotted()`** (`servo/selector_parser.rs`): the servo build's
    `PseudoElement` takes `::before`, `::after`, `::marker`, `::placeholder`, `::file-selector-button`
    and `::details-content` after `::slotted()` (CSS Scoping 1 §3.2.2), and `::marker` after
    `::before`/`::after` (CSS Pseudo 4 §4); upstream left the selectors crate's defaults (none).
15. **Implicit `@scope` roots in the servo build** (`stylesheets/stylesheet.rs`, `stylist.rs`):
    `Stylesheet::implicit_scope_root`, which the embedder sets from the sheet's owner node, is what
    `StylesheetInDocument::implicit_scope_root` returns (upstream: always `None`, so `@scope` without
    `<scope-start>` scoped to the document element; CSS Cascade 6 §2.1.1). The servo build keeps
    shadow-tree implicit roots as `StylistImplicitScopeRoot::Normal`, since a servo shadow tree's
    sheets are its own and its cascade data is not shared with another tree.

16. **CSS Overflow in the servo build** (numbered after agent/cssom's 14 and 15;
   `properties/longhands.toml`, `properties/shorthands.toml`, `style_adjuster.rs`,
   `values/specified/box.rs`, `values/computed/box.rs`): `scrollbar-gutter`,
   `content-visibility` (animated discretely), and the `scroll-margin-*` and `scroll-padding-*`
   longhands and their shorthands (CSS Scroll Snap 1 §5, used by `scrollIntoView`) were gecko-only;
   they now parse and compute behind `layout.unimplemented`. A `-webkit-box` (patch 11) is a legacy
   flex container whose children are flex items (`Display::is_item_container`, CSS Display 3
   Appendix A) and serializes as `-webkit-box`. `adjust_for_overflow` follows CSS Overflow 3 §3.1 as
   WPT tests it: `visible` computes to `auto` when the other axis is a scroll value, and `clip`
   stays `clip` (upstream also turned it into `hidden`). `overflow-clip-margin` takes a negative
   length too, as WPT's `css/css-overflow/parsing/overflow-clip-margin*.html` expect, and animates
   discretely (`css/css-overflow/overflow-no-interpolation.html`).
25. **`sideways-rl`, `sideways-lr` and `text-orientation` in the servo build** (`logical_geometry.rs`):
    upstream declares the sideways writing modes, and sets the writing mode's `UPRIGHT`/`TEXT_SIDEWAYS`
    flags from `text-orientation` (with `upright` forcing a used `direction: ltr`, CSS Writing Modes 4
    §5.1), for the gecko build only. snowlark-layout lays out all four vertical writing modes.
    `text-combine-upright` is a longhand of the servo build too (`properties/longhands.toml`, behind
    `layout.unimplemented`); snowlark-layout combines the text, so the gecko-only style adjustment that
    turns a text frame horizontal stays off.

26. **`:dir()` in the servo build** (`servo/selector_parser.rs`): `NonTSPseudoClass::Dir`, parsed and serialized like `:lang()`;
    snowlark-style's `match_non_ts_pseudo_class` works out the element's directionality. No element state
    bit, so a `dir` change on an ancestor does not invalidate `:dir()` in its descendants.

17. **SVG 2's properties in the servo build** (`properties/longhands.toml`, `properties/shorthands.toml`):
   `stop-color`, `stop-opacity`, `flood-color`, `flood-opacity`, `lighting-color`, `marker-start`,
   `marker-mid`, `marker-end` and `marker`, `text-anchor`, `paint-order`, `vector-effect`,
   `color-interpolation`, `color-interpolation-filters`, `shape-rendering`, `dominant-baseline`,
   `clip-rule` and `transform-box` were gecko-only; they now parse and compute behind
   `layout.unimplemented`, as the presentation attributes of SVG 2 §6.6 need them.
   `values/specified/transform.rs` has all the `transform-box` keywords in the servo build.


17. **Media Queries 4/5 features in the servo build** (`servo/media_features.rs`,
    `queries/feature_expression.rs`, `device/servo.rs`): the servo list had 15 features; it now also
    has `device-aspect-ratio`, `grid`, `color`, `color-index`, `monochrome`, `color-gamut`,
    `dynamic-range`, `video-dynamic-range`, `display-mode`, `prefers-reduced-motion`,
    `prefers-reduced-transparency`, `prefers-contrast`, `forced-colors`, `inverted-colors`,
    `overflow-block`, `overflow-inline`, `update`, `scripting` and `-webkit-transform-3d`, with what a
    desktop browser window on a standard display reports (8-bit sRGB, standard dynamic range, no
    user preference, `display-mode: browser`; print pages and does not update). `scripting` reads
    `Device::scripting_enabled` (set by snowlark-style from `DeviceConfig::scripting`). The list is a
    `LazyLock` because the new names are not static atoms of the servo build.
18. **calc() precision** (`values/specified/length.rs`, `values/specified/calc.rs`): absolute
    lengths convert to px in double precision, rounded once (`12cm` and `round(10cm, 6cm)` are
    the same px, as in engines that convert in doubles), and the sum of two percentages is kept to
    the seven significant digits single precision holds (`calc(60% - 50%)` is 10%, not
    10.0000024%: percentages are stored as fractions, which a decimal percentage is not exactly).
19. **CSS Values 4 §10.9.2 NaN rules** (`values/generics/calc.rs`): any NaN argument of `pow()`,
    `log()`, `round()`, `mod()` and `rem()` gives NaN (IEEE 754 has `pow(NaN, 0)` = 1), as do
    `pow(±1, ±infinity)` and `log()` with a base of 1 or below 0; a zero `log()` is 0⁺.
20. **`@supports font-format()`/`font-tech()` and `@font-face` `tech()` in the servo build**
    (`stylesheets/supports_rule.rs`): the servo evaluators answered false for everything; they now
    answer for what Snowlark loads and renders (OpenType, TrueType, collections, WOFF, WOFF2;
    OpenType features, variations, COLRv0, SVG and bitmap color glyphs). `eval_font_tech` is
    public so snowlark-style skips `@font-face` sources needing a technology it lacks (CSS Fonts 4
    §4.3.1).
21. **The `container` shorthand in the servo build** (`properties/shorthands.toml`,
    `properties/shorthands.rs`), behind the container queries preference like its longhands.
22. **Unknown container features select no container** (`stylesheets/container_rule.rs`,
    `queries/condition.rs`): CSS Conditional 5 §5.4; a `<container-query>` with a
    `<general-enclosed>` (other than one with `var()`, `attr()` or `env()`, which is parsed again
    once substituted) is unknown, so `((unknown) or (width))` does not match although Kleene `or`
    would make it true.
30. **`image-orientation` in the servo build** (`properties/longhands.toml`): upstream declares it for
   gecko only. CSS Images 3 §5.3 (`from-image` | `none`); snowlark-paint applies EXIF orientation when
   decoding and snowlark-render shows the stored pixels for `none` on same-origin images.

31. **`transform-box` in the servo build** (`properties/longhands.toml`): upstream declares it for
   gecko only. CSS Transforms 1 §6.2; snowlark-render resolves an SVG element's transform against it
   when it serializes SVG documents (inline `svg`, SVG images).
   `all_transform_boxes_are_enabled` (`values/specified/transform.rs`) is true: `content-box` and `stroke-box` parse.

32. **`object-view-box`** (`properties/longhands.toml`, `values/generics/basic_shape.rs`,
   `values/specified/basic_shape.rs`, `values/computed/basic_shape.rs`): a new longhand, `none |
   <basic-shape-rect>` computed as an `inset()` (CSS Images 5 §5.1); upstream has no such property.
   snowlark-layout crops a replaced element's natural size to it.

33. **`image-rendering` values** (`values/specified/image.rs`): `smooth`, `optimizeSpeed` and
   `optimizeQuality` parse in the servo build too, and `high-quality` (CSS Images 4 §5.4), which
   upstream lacks, in both; snowlark-render draws `crisp-edges`, `pixelated` and `optimizeSpeed` with
   nearest-neighbour sampling and the rest smoothly.

34. **`image()` with a source** (`values/generics/image.rs`, `values/specified/image.rs`,
   `values/computed/image.rs`): upstream parses only `image(<color>)`; the rest of CSS Images 4 §2.3,
   `image( <image-tags>? [ <image-src>? , <color>? ]! )`, is a new `Image::Fallback` variant, which
   snowlark-render shows as its source once loaded, else its color. WPT's
   `parsing/image-function-invalid.html` ("image(url(foo.png))") expects Chrome's `image(<color>)`
   only and fails against the grammar; `css-image-fallbacks-and-annotations003/004.html` use CSS
   Images 3's list of sources, which the grammar dropped. The tags parse and serialize but do not
   flip the image yet.

35. **`cross-fade()` serializes `<image> <percentage>`** (`values/generics/image.rs`): the fields of
   `GenericCrossFadeElement` are in that order, CSS Images 4 §2.6's canonical one (upstream wrote the
   percentage first). `CrossFadeElement::parse_percentage` rejects a literal outside `[0%,100%]`, which
   upstream clamped (`values/specified/image.rs`).

36. **Gradients with `none` default to Oklab** (`values/specified/image.rs` `has_modern_syntax`): a
   color with a missing component counts as modern for the default `<color-interpolation-method>`
   (CSS Color 4 §12.1: the legacy syntax has no `none`), as WPT's gradient-analogous-missing-components
   tests expect; an explicit `in srgb` keeps sRGB.

37. **CSS Motion Path in the servo build** (`properties/longhands.toml`, `properties/shorthands.toml`, `properties/shorthands.rs`):
   `offset-anchor`, `offset-distance`, `offset-position`, `offset-rotate` and the `offset` shorthand were
   gecko-only; they parse and compute behind `layout.unimplemented` as `offset-path` already did. Motion
   Path 1; snowlark-render's `motion.rs` applies them. `has_transform_or_perspective`
   (`servo/restyle_damage.rs`) counts `offset-path`, which makes a containing block as a transform does.

38. **`polygon(round <length>)` and fill rules of outlines** (`values/generics/basic_shape.rs`,
   `values/specified/basic_shape.rs`): `GenericPolygon::round`, CSS Shapes 2, which serializes after the
   fill rule, interpolates with none as zero and is `None` at zero; `polygon()` and `shape()` keep a
   `<fill-rule>` in `offset-path` too (`path()` still takes none there). WPT css/motion/parsing.

39. **Transitions skip unchanged properties before building their values** (`servo/animation.rs`
   `start_transitions_if_applicable`, `properties/helpers/animated_properties.mako.rs`
   `AnimationValue::computed_values_equal`): a property with no transition of its own whose
   before-change and after-change computed values are equal (the same style struct, or the
   longhand's `_equals`) starts nothing, so its two `AnimationValue`s are not built. As Blink's
   `CSSAnimations::CalculateTransitionUpdateForPropertyHandle` (`ComputedTransitionValuesEqual`).
   Only with the same effective zoom and font size, the other inputs of `to_animated_value`.
   `transition: all` lists every animatable longhand; on NewsSite's links this was 8 to 11% of the
   suite's time.

40. **`ComputedValues::longhand_equals`** (`properties/properties.mako.rs`): whether two styles have the
   same computed value of a longhand (the same style struct, or the longhand's `_equals`), for any
   non-logical longhand. snowlark-style's `computed::same_computed_value` uses it, so that the inline
   SVG writer skips a property equal to its parent's or the initial value without serializing it.

41. **`Animation::properties_changed()`** (`servo/animation.rs`): the properties a CSS animation
   changes, read-only, so that snowlark-style can tell which elements have a transform or opacity
   animation in effect (Blink's `UpdateAnimationFlagsForEffect`, the compositor's layers).

42. **`Animation::property_keyframes()` and `Transition::parts()`** (`servo/animation.rs`): a CSS
   animation's keyframes for one property (offset, value, timing function) and a transition's start
   and end values and timing function, read-only, so that snowlark-render can hand transform and opacity
   animations to the compositor (`docs/compositor-animations.md`, S2; Blink's
   `CompositorAnimations::StartAnimationOnCompositor`).

43. **`will-change: mask-image` is a backdrop root** (`values/specified/box.rs`
   `change_bits_for_longhand`): `WillChangeBits::BACKDROP_ROOT` for `mask-image` (and so the `mask`
   shorthand), as Filter Effects 2 §3.1 lists a mask among the backdrop roots; upstream set it for
   opacity, filters, clip-path, blend modes and view transitions only.

44. **`filter: url()` in the servo build** (`values/specified/effects.rs`, `url/servo.rs`): `filter` and
   `backdrop-filter` parse a `<url>` and compute it to a `ComputedUrl` (Filter Effects 1 §5, a reference
   to an SVG `<filter>` element); upstream parsed it only for gecko. `SpecifiedUrl` gains
   `to_computed_value_without_context` (a url's computed value needs no context). A list with a url
   still animates discretely (`#[animation(error)]`, Filter Effects 1 §14).

45. **Color mixing weights in f64** (`color/mix.rs`, `values/animated/color.rs`): `ColorMixItem::weight`
   is f64 and the mix sums and multiplies in f64 (`ColorMixItem::with_exact_weight`); the two color
   `Animate` impls pass the procedure's weights unrounded, and `Color::animate` mixes two absolute
   colors directly (as `from_color_mix` would). Upstream rounded `1 - p` and `p` to f32, which at a
   progress such as 2^31 sum to 0, and the mix skipped the second color: a color extrapolated far past
   the end gave the start color instead of the clamped one (the WPT `filter-interpolation-002`
   drop-shadow at 2147483648).
46. **`auto` in the `@font-face` style descriptors, and `oblique 0rad` as `normal`** (`font_face.rs`):
   `font-weight`, `font-style` and `font-width` take `auto`, their initial value (CSS Fonts 4 §4.4: the
   face is selected as `normal`, and variation axes are not clamped); `FontWeightRange` and
   `FontWidthRange` become enums with an `Auto` variant and `FontStyleRange` gains one, each computing to
   the normal range. `font-style: oblique` with both angles written as zero in any unit (`0rad`, `0turn`)
   serializes as `normal`, as `0deg` did; a `calc()` that computes to zero still does not.
47. **`:in-range` in the servo build** (`servo/selector_parser.rs`): the parser maps `in-range` to`n   `NonTSPseudoClass::InRange` (Selectors 4 §11.4); upstream listed `:out-of-range` only, so a rule with`n   `:in-range` was dropped as invalid.
48. **A transition keeps its last interpolated value** (`servo/animation.rs`, `PropertyAnimation`): the
   value last calculated is kept with the progress it was calculated at and returned for the same
   progress. A restyle asks for a running transition's value up to three times (its cascade, its
   before-change style, the traversal's animation rules) at the timeline's time, which does not move
   within a task, and a page that changes a style and reads layout in a loop (a menu toggled and
   measured) restyles its transitioning elements many times at one time: each ask interpolated
   transform lists and `calc()` values again. The kept value is dropped when a reversed transition's
   start value and duration change; clones start without one. (Returning the start or end value itself
   at progress 0 or 1 instead was tried and is wrong: the interpolation tests expect the interpolated
   form there, such as `rotate` with differing axes.)
49. **`transition-property: all` a style struct at a time** (`servo/animation.rs`,
   `start_transitions_if_applicable`; `AnimationValue::for_each_transition_all_candidate` in
   `properties/helpers/animated_properties.mako.rs`): with `all` as the only transition property, the
   longhands of `all` (listed by style struct) are taken only from the structs that differ between the
   before-change and after-change styles, plus the properties of the element's transitions, in the
   same order as before. Each restyle of an element with `transition: all` collected, mapped and
   tested every longhand of `all` (about 300) to start nothing for the unchanged ones.
50. **`Stylesheet::shared_contents`** (`stylesheets/stylesheet.rs`): whether the sheet's contents may be
   shared with other sheets. The style engine parses a `style` element's text once per document and
   gives every `style` element with the same text (and base URL) a sheet over the same contents, so
   identical shadow trees also share their cascade data through `author_data_cache`; a sheet so made
   gets a deep copy of its own (`deep_clone`) before the CSSOM first reaches its rules.
51. **`@keyframes` as tree-scoped references** (`stylist.rs` `Stylist::lookup_keyframes_for_style`,
   `servo/animation.rs` `maybe_start_animations`): CSS Scoping 1 §3.5. The keyframes an
   `animation-name` refers to are looked up from the tree of the declaration that set it (found on the
   style's rule node; its cascade level gives the shadow root through `get_shadow_root_for_scoped`),
   then from each tree around that one out to the document, then the user and user-agent sheets.
   `lookup_keyframes` looked from the element's own trees, so a document rule's name found the host's
   shadow tree's keyframes, and a shadow tree's rule never found the document's.
52. **`:has-slotted` in the servo build** (`servo/selector_parser.rs`): the parser maps `has-slotted`
   to `NonTSPseudoClass::HasSlotted` (CSS Scoping 1), matched by the `HAS_SLOTTED` element state that
   the style engine sets on a slot with flattened assigned nodes.
53. **Inert subtrees in the servo build** (`properties/longhands.toml`, `servo/selector_parser.rs`): HTML
   §6.3. The internal inherited `-moz-inert` property (UA sheets only) is built for servo too, and the
   servo parser takes `:-moz-inert` and `:-moz-topmost-modal` in UA sheets only, matched by the `INERT`
   and `TOPMOST_MODAL` element states. Snowlark's UA sheet sets the property from the `inert` attribute,
   the root of a document blocked by a modal dialog (`INERT`) and the blocking dialog (`TOPMOST_MODAL`),
   so inertness follows the flat tree; hit testing and selection read it.
54. **Hyphenation and `text-wrap` in the servo build** (`properties/longhands.toml`,
   `properties/shorthands.toml`, `properties/shorthands.rs`): upstream declares `hyphens`,
   `hyphenate-character`, `hyphenate-limit-chars`, `text-wrap-style` and the `text-wrap` shorthand
   for the gecko engine only, though their value types are engine-neutral (CSS Text 3 and 4).
   `text-wrap-style` also takes `pretty` (CSS Text 4), which upstream lacks. `hyphens` drops Gecko's `-moz-`
   alias (the bindings expose no `-moz-` name, so `CSS.supports` must not take one). snowlark-layout reads `hyphens` and `hyphenate-character` (a line broken at a soft hyphen
   ends with the hyphenation character).
55. **`text-indent` keywords and `text-align-last: match-parent`** (`values/specified/text.rs`): the
   servo build parsed `text-indent`'s `hanging` and `each-line` (CSS Text 3 §7.1) as errors;
   snowlark-layout implements them. `TextAlignLast` gets `match-parent` (CSS Text 3 §7.3), which
   computes like `text-align`'s: the parent's value, with `start` and `end` resolved against the
   parent's direction to `left` or `right`.
56. **Column rules and `column-fill` in the servo build** (`properties/longhands.toml`,
   `properties/shorthands.toml`, `properties/shorthands.rs`): upstream declares `column-rule-color`,
   `column-rule-style`, `column-rule-width`, `column-fill` and the `column-rule` shorthand for the
   gecko engine only, though their value types are engine-neutral (CSS Multi-column Layout 1 §4, §7.1).
   The `column-rule` shorthand serializes its shortest form: width, style, color, leaving out the
   longhands at their initial values (`medium` when all are).
57. **`column-height` and `column-wrap`** (`properties/longhands.toml`, `properties/shorthands.toml`,
   `properties/shorthands.rs`): CSS Multi-column Layout 2, which upstream lacks. `columns` takes
   `[ <'column-width'> || <'column-count'> ] [ / <'column-height'> ]?` and resets `column-wrap`; it
   serializes as the empty string while `column-wrap` is not `auto`. Snowlark does not lay out
   multi-column containers yet, so neither property changes layout.
58. **Breaks, orphans and widows in the servo build** (`properties/longhands.toml`, `properties/shorthands.toml`,
   `values/specified/box.rs`): upstream declares `break-before`, `break-after`, `break-inside` and the legacy
   `page-break-*` shorthands for the gecko engine only, and its `BreakBetween` lacks `avoid-page`, `recto`, `verso`,
   `avoid-column` and `column` (CSS Fragmentation 3 §3.1; the legacy shorthands take none of them and serialize
   them as the empty string). `orphans` and `widows` (§3.3: inherited positive integers, initially 2) are new.
   Multi-column layout uses all of them. They leave the counted unknown properties
   (`properties/counted_unknown_properties.py`), and `GreaterThanOrEqualToOne` derives `ToTyped`
   (`values/generics/mod.rs`) for them.
59. **`font-variant-caps` values in the servo build** (`properties/longhands.toml`): upstream gives the servo
   engine only `normal` and `small-caps`; `all-small-caps`, `petite-caps`, `all-petite-caps`, `unicase` and
   `titling-caps` (CSS Fonts 4 §6.6) are valid there too now. The `font` shorthand still takes only
   `small-caps` and serializes as the empty string for the others (its existing check).
60. **`font-synthesis` in the servo build** (`properties/longhands.toml`, `properties/shorthands.toml`,
   `properties/shorthands.rs`, `values/specified/font.rs`): upstream declares the shorthand and its
   `font-synthesis-style`, `-small-caps` and `-position` longhands for the gecko engine only (CSS Fonts 4 §5.2-5.6;
   `font-synthesis-weight` was already there). `FontSynthesisStyle` derives `Serialize`, `Deserialize` and `Hash`
   as the servo font struct needs.
61. **System font keywords in the servo build** (`values/specified/font.rs`, `values/computed/font.rs`,
   `properties/shorthands.rs`): upstream's servo `SystemFont` is an empty enum, so `font: caption` (and `icon`,
   `menu`, `message-box`, `small-caption`, `status-bar`; CSS Fonts 4 §3.7.1) was invalid. It now has those six
   keywords, the `font` shorthand parses and serializes them as gecko does (each longhand holds the keyword), and
   the longhands compute them directly from `SystemFont`'s methods instead of gecko's cached platform font:
   `menu`, `small-caption` and `status-bar` are Segoe UI at 12px (Windows' defaults for those fonts), the others
   Arial at the `medium` size, every other longhand initial. `FontFamily::for_system_font` is in both builds.
62. **`font-palette` with `palette-mix()` in the servo build** (`properties/longhands.toml`,
   `values/generics/font.rs`, `values/specified/font.rs`, `values/computed/font.rs`, `color/mix.rs`): upstream
   declares `font-palette` for the gecko engine only, as one identifier, not animatable. It is now
   `normal | light | dark | <dashed-ident> | palette-mix()` (CSS Fonts 4 §9.1-9.2) in both builds:
   `palette-mix( <color-interpolation-method>? , [ <font-palette> && <percentage [0,100]>? ]# )`, its
   percentages normalized and serialized as `color-mix()`'s are (omitted ones filled in at parse time unless
   one is a calc(), and at computed-value time always; all omitted when they are all 100%/N; `in oklab`
   omitted). It interpolates as `palette-mix(in oklab, A (1-p), B p)` between the end points (equal values
   and progress outside (0, 1) give an end point; addition is not defined). `HueInterpolationMethod` and
   `ColorInterpolationMethod` derive `Serialize` and `Deserialize`, as the servo font struct needs.
   `@font-palette-values` needs the `layout.css.font-palette.enabled` preference (snowlark-style turns it on).
63. **`font-variant-emoji` in the servo build** (`properties/longhands.toml`, `properties/shorthands.toml`,
   `properties/shorthands.rs`): upstream declares it (CSS Fonts 4 §6.13: `normal | text | emoji | unicode`) for
   the gecko engine only. It is now a longhand of `font-variant` (which parses and serializes it) and is reset
   by `font` in both builds.
64. **Integers interpolate at `<number>` precision** (`values/animated/mod.rs`): `Animate for i32` rounded the
   `f64` result half up, but the progress comes from easing arguments parsed as `f32`s, so a value meant to be a
   half came out just under it (`column-count` from 10 to 1 at -0.5 through `cubic-bezier(0,
   -0.8333333333333334, 1, -0.8333333333333334)`: 14.4999998, rounded to 14 instead of 15). The value is
   rounded to `f32` first (below 2^24, where `f32` holds every integer), as `Animate for f32` does.
66. **Computed styles kept across restyles** (`rule_cache.rs` `MatchedPropertiesCache`, `style_resolver.rs`,
   `context.rs`, `driver.rs`, `device/mod.rs` `snowlark_root_and_viewport`, `properties.mako.rs`
   `snowlark_inherits_equal`): the rule cache lives for one traversal and keeps only reset structs, so an element
   inserted in a later restyle cascaded again whatever matched it before. The embedder now owns a cache from
   rule node to whole styles, with the element's name, its parent's inherited values and its layout parent's
   display and writing mode; a cascade with all of them equal returns a copy of the kept style instead (its
   structs shared, the `Arc` its own: the style sharing cache takes children of parents with one style `Arc` for
   cousins whose parents matched the same selectors, and the elements given one kept style can differ in
   classes; one `Arc` for all of them made `.max-width > .stretch` children take a `.min-width > .stretch`
   child's style). Threads keep new
   entries in their thread-local context and the driver hands them to `SharedStyleContext::matched_properties_sink`
   at the end of the traversal. Styles that read anything else (container queries and units, sibling counts,
   `attr()`, font metrics, an inherited reset property, the root, links, `@starting-style`, pseudo-elements) are
   not kept; a change of the root's font values, the viewport or the body color misses and empties it; the
   embedder empties it on any change that can restyle every element.
67. **Style sharing candidates tested from cached facts** (`sharing/mod.rs` `ValidationData::parent`,
   `test_candidate`; `sharing/checks.rs` `parents_allow_sharing`): the parent of the element and of each
   candidate, and whether each parent lets its children share with cousins, were found again for every
   candidate tested (two flat-tree parent lookups and two data borrows per test; a chart page tests about
   25 candidates per element). They are found once per element with the parent's style identity and kept in
   its validation data. The cached class lists and presentational hints are compared before the checks
   that query both elements again per candidate (state, link, shadow roots, slots, animations, id rules,
   style attribute); the checks are all required, so their order changes no outcome.
68. **No sharing lookup for presentational hints of an element's own** (`dom.rs`
   `TElement::has_unshared_presentational_hints`, `sharing/mod.rs` `share_style_if_possible`,
   `lookup_by_rules`): hint blocks compare by identity, and an SVG chart's shapes each have a block no
   other element holds (their own `d`, `x`, `y`), yet every one tested the 25-31 candidates of the cache
   at its depth. The embedder says when an element's block is its alone; neither lookup is made for it
   (it is still inserted, for an element given its block later). No style changes: a lookup that
   cannot hit is skipped.
69. **No cascade for a `::before` or `::after` its rules cannot generate** (`style_resolver.rs`
   `before_or_after_may_be_generated`, `resolve_pseudo_style`, `cascade_styles_with_default_parents`):
   a rule such as `*, ::before, ::after { box-sizing: border-box }` matches both pseudo-elements of
   every element, and each was cascaded only to be dropped (`content` computes to `normal`). Unless
   the styles are forced, an eager `::before` or `::after` is cascaded only when a rule matched for
   it (normal or visited) declares `content` (`all` included), or declares `display` with
   something other than a plain value (`inherit`, `var()`: such styles are kept whatever their
   `content`). No style changes: only styles that were dropped are not computed.
70. **The parts of `progress` and `meter`** (`servo/selector_parser.rs`): the lazily cascaded
   pseudo-elements `::-webkit-progress-inner-element`, `::-webkit-progress-bar`,
   `::-webkit-progress-value`, `::-webkit-meter-inner-element`, `::-webkit-meter-bar`,
   `::-webkit-meter-optimum-value`, `::-webkit-meter-suboptimum-value` and
   `::-webkit-meter-even-less-good-value`, which pages style and the embedder lays out as the parts of
   an element with `appearance: none`.
71. **The caller styles the top of a parallel traversal** (`driver.rs`, `scoped_tls.rs`): a traversal
   started from a thread outside the pool ran entirely on a worker (`spawn_fifo` of the whole
   closure) while the caller only waited, so every traversal given the pool paid a worker's wake-up
   and the caller's own wake-up at the end. The closure now runs in place on the caller, which has
   a slot of its own in `ScopedTLS` (one more than the pool's threads) and the workers' local queue
   size; work goes to the pool only when a level of the tree has enough elements to share, as the
   traversal does on a thread of the pool. The embedder can then offer the pool to every restyle.
72. **Running transitions applied over a kept base style** (`style_resolver.rs`
   `cascade_style_and_visited`, `properties/cascade.rs` `apply_transitions_to_base`): an element with
   a running transition was cascaded in full on every restyle (patch 66 keeps no style with animation
   rules). Its rules without the transitions (`remove_animation_rules`) are its after-change rules,
   whose style patch 66 keeps; the element now gets that style with the transitions' computed values
   applied over it, as the cascade ends with the transitions origin. Only when the rules hold no
   other animation level and every transitioning property is one no other property of the element
   reads or adjusts (opacity, transforms, colors, shadows, filters, visibility, sizes, insets,
   margins, paddings, radii, background position and size, clip path, spacing, SVG paint, flex and
   gap; not the font properties, `line-height`, `zoom`, the writing mode, `display`, `position`,
   `float`, border or outline styles, `overflow`, containment). Otherwise the rules are cascaded as
   before. No style changes.

73. **Unusual elements' `display: contents` in the servo build** (`style_adjuster.rs`, CSS Display 3
   §2.5, "Unboxing"): upstream computes `display: contents` to `none` for the elements that cannot be
   unboxed (`br`, `wbr`, `meter`, `progress`, `canvas`, `embed`, `object`, `audio`, `iframe`, `img`,
   `video`, `frame`, `frameset`, `input`, `textarea`, `select`; SVG elements other than a nested `svg`,
   `a`, `g`, `use`, `tspan` and `textPath`; MathML elements) only in the gecko build. The servo build now
   does the same, comparing local names as strings; pseudo-elements keep `contents`.

74. **The highlight pseudo-elements** (`servo/selector_parser.rs`, `properties/data.py`, `properties/mod.rs`,
   `properties/properties.mako.rs`; CSS Pseudo-Elements 4 §3): the servo build parses `::target-text`,
   `::spelling-error` and `::grammar-error` (lazy, like the other pseudo-elements the traversal does not
   style) and has `is_highlight_pseudo()`. A new property flag, `APPLIES_TO_HIGHLIGHT` (`color`,
   `background-color`, `text-decoration` and its longhands, `text-underline-offset`, `text-underline-position`,
   `text-shadow`, `fill`, `stroke`, `stroke-width`; §3.2), is the property restriction of those three and of
   `::selection`, so their cascade ignores every other property (custom properties pass, as for the other
   restrictions). `pseudo_none_array` is built with `std::array::from_fn`: with more than 32 pseudo-elements
   the array has no `Default`.

75. **`Highlight` and `HighlightText`** (`device/servo.rs`, CSS Color 4 §6.2): in the light color scheme the
   system colors of selected text are Windows' default selection colors, rgb(0, 120, 215) and white (upstream:
   rgb(0, 65, 198) and black), the colors the engine paints a selection in where no `::selection` colors are
   declared. The dark scheme keeps upstream's values.

76. **`word-break: break-word` in the servo build** (`values/specified/text.rs`, CSS Text 3 §5.2): upstream
   parses the value only with the `gecko` feature. It is the deprecated value the spec keeps for
   compatibility: `word-break: normal` with `overflow-wrap: anywhere`, which snowlark-layout's line
   breaking applies.

77. **`::-webkit-` pseudo-elements and legacy pseudo-class names** (`servo/selector_parser.rs`, Selectors 4
   §compat): any `::-webkit-` pseudo-element is valid. The form control and scrollbar parts pages style under
   these names (`::-webkit-inner-spin-button`, `::-webkit-scrollbar-thumb` and others, by name) and any other
   `-webkit-` name (`UnknownWebkit`, serialized as `::-webkit-unknown`; not supported in `@supports`, as in
   Gecko) parse as pseudo-elements the stylist drops (`is_unknown_webkit_pseudo_element`), after which user
   action pseudo-classes may follow. `::-webkit-input-placeholder` and `::-webkit-file-upload-button` are
   `::placeholder` and `::file-selector-button`; `:-webkit-autofill`, `:-webkit-full-screen` and
   `:-webkit-any-link` are `:autofill`, `:fullscreen` and `:any-link`.
78. **Custom scroll bar parts** (`servo/selector_parser.rs`): `::-webkit-scrollbar`, `-thumb`, `-track`,
   `-track-piece`, `-button` and `-corner` are lazily cascaded pseudo-elements (`is_webkit_scrollbar_part`),
   no longer among the parts patch 77 drops: the engine reads them from a scroll container to draw its scroll
   bars. A user action pseudo-class may follow them; their styles are cascaded without state, so the stylist
   (`stylist.rs` `add_styles`) leaves such rules out.
79. **`resize` in the servo build** (`properties/longhands.toml`, CSS UI 4 §5.1): upstream builds the
   longhand only for the `gecko` engine. The computed value (logical `block`/`inline` resolved against the
   writing mode) is upstream's; snowlark-layout reads it to place the resizer of a scroll container.
80. **Gradient text in the servo build** (`values/specified/background.rs`, `properties/longhands.toml`): upstream
   parses `background-clip: text` (CSS Backgrounds 4 §2.1) and builds `-webkit-text-fill-color` (Compat §5.4)
   only with the `gecko` feature. Both are in the servo build: snowlark-layout clips a `text` layer to the
   glyphs of the box's text and fills glyphs with the fill colour.
81. **The legacy `-webkit-box` properties in the servo build** (`properties/longhands.toml`,
   `properties/computed_value_flags.rs`, `style_adjuster.rs`): upstream has `-webkit-box-align`, `-direction`,
   `-pack`, `-flex` and `-ordinal-group` only as Gecko aliases of its XUL properties. They are servo longhands
   in the box struct, as `-webkit-box-orient` is (patch 11). An element whose layout parent is a `-webkit-box`
   gets the flag `IS_WEBKIT_BOX_ITEM`, so snowlark-render can give it the legacy box's item rules (its flex
   from `-webkit-box-flex`, its order from `-webkit-box-ordinal-group`) without changing computed values.
82. **The legacy `-webkit-` names of logical, multi-column and fragmentation properties**
   (`properties/longhands.toml`, `properties/shorthands.toml`, `properties/counted_unknown_properties.py`,
   `values/specified/length.rs`): upstream has none
   of them. They are aliases, as the web relies on: `-webkit-margin-`/`-padding-`/`-border-` `start`/`end`/
   `before`/`after` (and the borders' `-color`/`-style`/`-width`) of the inline and block start and end
   properties, `-webkit-column-count`/`-gap`/`-width`/`-span`/`-rule` (and its longhands)/`-webkit-columns`,
   `-webkit-box-decoration-break` and `-webkit-writing-mode`. Sizes take `-webkit-min-content`,
   `-webkit-max-content` and `-webkit-fit-content` as the keywords without the prefix (as `-moz-` ones).
83. **`-webkit-focus-ring-color`, `-webkit-tap-highlight-color`, `-webkit-font-smoothing` and
   `text-size-adjust`** (`values/specified/color.rs`, `device/servo.rs`, `properties/longhands.toml`,
   `properties/shorthands.rs`, `properties/counted_unknown_properties.py`, `values/*/text.rs`, Compat and CSS
   Size Adjust 1): the servo `SystemColor` gets `-webkit-focus-ring-color` (rgb(16, 16, 16), or
   rgb(238, 238, 238) in a dark scheme), which `outline-color` and `outline` take (`Color::parse_outline_color`)
   and any colour takes in a UA sheet or in quirks mode. `-webkit-tap-highlight-color` (inherited colour,
   initially 18% black) and `-webkit-font-smoothing` (`auto | none | antialiased | subpixel-antialiased`) are
   inherited servo longhands that nothing reads yet; `text-size-adjust` (`auto | none | <percentage [0,∞]>`,
   alias `-webkit-text-size-adjust`, `GenericTextSizeAdjust`) is one too: text autosizing is for small
   screens, so no desktop layout changes with it.
84. **`-webkit-user-drag` in the servo build** (`properties/longhands.toml`,
   `properties/counted_unknown_properties.py`, Compat): a servo longhand of the box struct, `auto | none |
   element`, not inherited. snowlark-page reads it to choose what a press drags: `element` makes a box a drag
   source, `none` keeps an image, a link or a `draggable` element from being one.
85. **Scroll snap properties in the servo build** (`properties/longhands.toml`, `values/specified/box.rs`, CSS
   Scroll Snap 1): `scroll-snap-type`, `scroll-snap-align` and `scroll-snap-stop` lose their gecko-only engine
   line, and `ScrollSnapType` (`axis`, `strictness`) and `ScrollSnapAlign` (`block`, `inline`) get public
   getters. snowlark-page snaps scrolls with them; `scroll-margin` and `scroll-padding` were already there.

To move to a newer stylo: replace this directory with the new release, re-apply the changes that upstream
still lacks, and run `cargo test -p snowlark-style`.
