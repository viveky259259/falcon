//! Dart expressions evaluated inside the running app to find and drive widgets.
//!
//! Every expression is a single immediately-invoked closure evaluated in the
//! scope of `package:flutter/src/widgets/binding.dart`, whose imports put the
//! widget, rendering, gesture and services libraries in scope. Results come
//! back as one string: records separated by `\x1e`, fields by `\x1f`. Plain
//! strings avoid depending on `dart:convert` being in scope.

use super::Finder;

pub const FIELD_SEP: char = '\u{1f}';
/// Result of an expression that ran while the app was mid-frame.
pub const BUSY: &str = "BUSY";
pub const RECORD_SEP: char = '\u{1e}';

/// Quote a Rust string as a Dart single-quoted string literal.
pub fn dart_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('\'');
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\'' => out.push_str("\\'"),
            '$' => out.push_str("\\$"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            c => out.push(c),
        }
    }
    out.push('\'');
    out
}

/// Helpers shared by every expression. Defines `sep`, `rs`, `binding`,
/// `screen`, `dpr` and the widget accessors used by finders.
///
/// The VM can run an `evaluate` at an interrupt point in the middle of a
/// frame, where reading render boxes trips layout assertions. The prelude
/// returns `BUSY` then, and the caller retries; see [`BUSY`].
const PRELUDE: &str = r#"
  if (SchedulerBinding.instance.schedulerPhase != SchedulerPhase.idle ||
      RenderObject.debugActiveLayout != null || RenderObject.debugActivePaint != null ||
      (WidgetsBinding.instance.buildOwner?.debugBuilding ?? false)) return 'BUSY';
  final sep = '\u001f';
  final rs = '\u001e';
  String clean(Object? o) => '${o ?? ''}'.replaceAll(sep, ' ').replaceAll(rs, ' ').replaceAll('\n', ' ');
  String tn(Widget w) => '${w.runtimeType}'.split('<').first;
  String? txt(Widget w) {
    final t = tn(w);
    try {
      if (t == 'Text') {
        final d = (w as dynamic).data;
        if (d != null) return '$d';
        return (w as dynamic).textSpan?.toPlainText();
      }
      if (t == 'RichText') return (w as dynamic).text.toPlainText();
      if (t == 'EditableText') return (w as dynamic).controller.text;
    } catch (_) {}
    return null;
  }
  String? keyOf(Widget w) {
    final k = w.key;
    if (k == null) return null;
    if (k is ValueKey) return '${k.value}';
    return '$k';
  }
  String? tipOf(Widget w) {
    try { if (tn(w) == 'Tooltip') return '${(w as dynamic).message}'; } catch (_) {}
    return null;
  }
  String? lblOf(Widget w) {
    try { if (tn(w) == 'Semantics') return (w as dynamic).properties.label; } catch (_) {}
    return null;
  }
  final binding = WidgetsBinding.instance;
  final view = binding.platformDispatcher.views.first;
  final dpr = view.devicePixelRatio;
  final screen = view.physicalSize / dpr;
  Offset? hitPoint(RenderBox ro, Rect r) {
    final v = r.intersect(Offset.zero & screen);
    if (v.width <= 0 || v.height <= 0) return null;
    final points = [v.center, Offset(v.left + v.width * 0.2, v.center.dy),
      Offset(v.left + v.width * 0.8, v.center.dy), Offset(v.center.dx, v.top + v.height * 0.25),
      Offset(v.center.dx, v.top + v.height * 0.75)];
    for (final p in points) {
      final res = HitTestResult();
      binding.hitTestInView(res, p, view.viewId);
      if (res.path.any((e) => e.target == ro)) return p;
    }
    return null;
  }
  String hit(RenderBox ro, Rect r) {
    try {
      return hitPoint(ro, r) != null ? '1' : '0';
    } catch (_) {
      return '?';
    }
  }
  Offset tapPoint(RenderBox ro, Rect r) {
    try {
      return hitPoint(ro, r) ?? r.center;
    } catch (_) {
      return r.center;
    }
  }
  String? labelIn(Element e) {
    String? found;
    void f(Element x) {
      if (found != null) return;
      final w = x.widget;
      final s = tipOf(w) ?? txt(w) ?? lblOf(w);
      if (s != null && s.trim().isNotEmpty) { found = s; return; }
      x.visitChildren(f);
    }
    f(e);
    return found;
  }
  final root = binding.rootElement;
  if (root == null) return 'ERR${sep}the app has no root element yet';
"#;

impl Finder {
    /// A Dart boolean expression over `w` (a `Widget`).
    pub fn dart_predicate(&self) -> String {
        let mut parts = Vec::new();
        if let Some(t) = &self.text {
            parts.push(format!("txt(w) == {}", dart_str(t)));
        }
        if let Some(t) = &self.text_contains {
            parts.push(format!("(txt(w)?.contains({}) ?? false)", dart_str(t)));
        }
        if let Some(k) = &self.key {
            parts.push(format!("keyOf(w) == {}", dart_str(k)));
        }
        if let Some(t) = &self.type_name {
            parts.push(format!("tn(w) == {}", dart_str(t)));
        }
        if let Some(t) = &self.tooltip {
            parts.push(format!("tipOf(w) == {}", dart_str(t)));
        }
        if let Some(l) = &self.label {
            parts.push(format!("lblOf(w) == {}", dart_str(l)));
        }
        if parts.is_empty() {
            "false".to_string()
        } else {
            parts.join(" && ")
        }
    }
}

/// Find widgets matching `finder`, outermost match only. Returns `OK` then one
/// record per candidate: `type, label, x, y, w, h, hittable`. A candidate is
/// hittable when a pointer at its centre or one of four inner points reaches
/// it (overlays often cover only the centre); `c` is that point. When `act_index`
/// is set, runs `action` on that candidate with `el`, `ro` and `c` (centre,
/// logical pixels) in scope; `action` must return a string.
pub fn find_expression(
    finder: &Finder,
    onscreen_only: bool,
    act_index: Option<usize>,
    action: &str,
) -> String {
    let act = match act_index {
        Some(i) => format!(
            "if ({i} >= cands.length) return 'ERR${{sep}}match {i} is gone';\n  \
             final el = cands[{i}].el; final ro = cands[{i}].ro; final c = cands[{i}].c;\n  {action}"
        ),
        None => String::new(),
    };
    format!(
        r#"(() {{
{PRELUDE}
  final found = <Element>[];
  void visit(Element e) {{
    final w = e.widget;
    if ({pred}) {{ found.add(e); return; }}
    e.visitChildren(visit);
  }}
  visit(root);
  final cands = <({{Element el, RenderBox ro, Offset c, Rect r}})>[];
  for (final e in found) {{
    final ro = e.renderObject;
    if (ro is! RenderBox || !ro.attached || !ro.hasSize) continue;
    final r = ro.localToGlobal(Offset.zero) & ro.size;
    if ({onscreen} && !r.overlaps(Offset.zero & screen)) continue;
    cands.add((el: e, ro: ro, c: tapPoint(ro, r), r: r));
  }}
  {act}
  final out = <String>['OK'];
  for (final k in cands) {{
    out.add([tn(k.el.widget), clean(labelIn(k.el)), k.r.left.toStringAsFixed(1), k.r.top.toStringAsFixed(1),
      k.r.width.toStringAsFixed(1), k.r.height.toStringAsFixed(1), hit(k.ro, k.r)].join(sep));
  }}
  return out.join(rs);
}})()"#,
        pred = finder.dart_predicate(),
        onscreen = onscreen_only,
    )
}

/// Pointer down and/or up at `c`, using pointer id `pid`.
///
/// The events are dispatched from a timer, outside the evaluation, so
/// handlers never run with evaluator frames on the stack (Flutter's error
/// reporting can't parse those) or in the middle of a frame.
pub fn pointer_action(pid: u32, down: bool, up: bool) -> String {
    let mut s = String::from("Timer.run(() {\n");
    if down {
        s.push_str(&format!(
            "GestureBinding.instance.handlePointerEvent(PointerDownEvent(pointer: {pid}, position: c));\n"
        ));
    }
    if up {
        s.push_str(&format!(
            "GestureBinding.instance.handlePointerEvent(PointerUpEvent(pointer: {pid}, position: c));\n"
        ));
    }
    s.push_str("});\n");
    s.push_str("return 'DONE${sep}${c.dx.toStringAsFixed(1)}${sep}${c.dy.toStringAsFixed(1)}';");
    s
}

/// Pointer down and/or up at a raw logical-pixel position.
pub fn pointer_at_expression(pid: u32, x: f64, y: f64, down: bool, up: bool) -> String {
    format!(
        "(() {{\n{PRELUDE}\n  final c = Offset({x:.1}, {y:.1});\n  {}\n}})()",
        pointer_action(pid, down, up)
    )
}

/// Focus the text field at the match and replace its text.
pub fn type_action(pid: u32, text: &str, submit: bool) -> String {
    format!(
        r#"List<Element> fields(Element root) {{
    final out = <Element>[];
    void f(Element e) {{
      if (tn(e.widget) == 'EditableText') {{ out.add(e); return; }}
      e.visitChildren(f);
    }}
    f(root);
    return out;
  }}
  Element? ed;
  var many = false;
  void pick(List<Element> xs) {{ if (xs.length == 1) ed = xs.first; if (xs.length > 1) many = true; }}
  pick(fields(el));
  if (ed == null && !many) {{
    var depth = 0;
    el.visitAncestorElements((a) {{
      pick(fields(a));
      depth++;
      return ed == null && !many && depth < 8;
    }});
  }}
  if (many) return 'ERR${{sep}}several text fields are near the match; find the field itself (its key, hint text or type TextField)';
  if (ed == null) return 'ERR${{sep}}no text field at or around the match';
  final fr = ed!.renderObject;
  final fc = fr is RenderBox && fr.hasSize ? fr.localToGlobal(fr.size.center(Offset.zero)) : c;
  final st = (ed! as StatefulElement).state as dynamic;
  final t = {text};
  Timer.run(() {{
    GestureBinding.instance.handlePointerEvent(PointerDownEvent(pointer: {pid}, position: fc));
    GestureBinding.instance.handlePointerEvent(PointerUpEvent(pointer: {pid}, position: fc));
    st.updateEditingValue(TextEditingValue(text: t, selection: TextSelection.collapsed(offset: t.length)));
    if ({submit}) st.performAction(TextInputAction.done);
  }});
  return 'DONE${{sep}}${{fc.dx.toStringAsFixed(1)}}${{sep}}${{fc.dy.toStringAsFixed(1)}}';"#,
        text = dart_str(text),
    )
}

/// Scroll the match into view.
pub const SHOW_ACTION: &str =
    "Timer.run(() => ro.showOnScreen());\n  return 'DONE${sep}${c.dx.toStringAsFixed(1)}${sep}${c.dy.toStringAsFixed(1)}';";

/// Scroll the first on-screen `Scrollable` that can scroll (inside `start`,
/// or anywhere) along its axis by `dx`/`dy` logical pixels, or by that many
/// viewport heights when `pages` is set.
pub fn scroll_by_expression(start: Option<&Finder>, dx: f64, dy: f64, pages: bool) -> String {
    let start_code = match start {
        Some(f) => format!(
            "Element? start; void sf(Element e) {{ if (start != null) return; final w = e.widget; \
             if ({}) {{ start = e; return; }} e.visitChildren(sf); }} sf(root);\n  \
             if (start == null) return 'ERR${{sep}}no widget matches `within`';",
            f.dart_predicate()
        ),
        None => "Element? start = root;".to_string(),
    };
    format!(
        r#"(() {{
{PRELUDE}
  {start_code}
  Element? sc;
  void g(Element e) {{
    if (sc != null) return;
    if (tn(e.widget) == 'Scrollable') {{
      final r = e.renderObject;
      final p = ((e as StatefulElement).state as dynamic).position;
      final canScroll = p.hasContentDimensions && p.maxScrollExtent > p.minScrollExtent;
      if (canScroll && r is RenderBox && r.hasSize && (r.localToGlobal(Offset.zero) & r.size).overlaps(Offset.zero & screen)) {{ sc = e; return; }}
    }}
    e.visitChildren(g);
  }}
  g(start!);
  if (sc == null) return 'ERR${{sep}}nothing on screen can scroll';
  final st = (sc! as StatefulElement).state as dynamic;
  final pos = st.position;
  final dir = '${{st.widget.axisDirection}}';
  final vertical = dir.contains('down') || dir.contains('up');
  final double step = vertical ? {dy:.3} : {dx:.3};
  final double d = {pages} ? step * pos.viewportDimension : step;
  final double before = pos.pixels;
  final double to = (before + d).clamp(pos.minScrollExtent, pos.maxScrollExtent).toDouble();
  Timer.run(() => pos.jumpTo(to));
  return 'DONE${{sep}}${{before.toStringAsFixed(1)}}${{sep}}${{to.toStringAsFixed(1)}}';
}})()"#
    )
}

/// Pop the current route, like the system back button.
pub fn back_expression() -> String {
    format!(
        "(() {{\n{PRELUDE}\n  Timer.run(() => binding.handlePopRoute());\n  return 'DONE';\n}})()"
    )
}

/// Describe what is on screen: text, buttons, fields and controls, outermost
/// first. First record is `SCREEN, width, height, dpr`; then one record per
/// element: `type, label, key, x, y, w, h, hittable`.
pub fn describe_expression(limit: usize) -> String {
    format!(
        r#"(() {{
{PRELUDE}
  bool interesting(Widget w) {{
    final t = tn(w);
    return t == 'Text' || t == 'RichText' || t == 'EditableText' || t == 'Tooltip' ||
      t.endsWith('Button') || t == 'Checkbox' || t == 'Switch' || t == 'Radio' || t == 'Slider' ||
      t == 'ListTile' || t == 'TextField' || t == 'TextFormField' || t == 'Tab' || t.endsWith('Chip') ||
      t == 'Image' || t == 'Icon';
  }}
  final out = <String>[['SCREEN', screen.width.toStringAsFixed(1), screen.height.toStringAsFixed(1), dpr.toStringAsFixed(2)].join(sep)];
  var n = 0;
  void visit(Element e) {{
    if (n >= {limit}) return;
    final w = e.widget;
    if (interesting(w)) {{
      final ro = e.renderObject;
      if (ro is RenderBox && ro.attached && ro.hasSize) {{
        final r = ro.localToGlobal(Offset.zero) & ro.size;
        if (r.overlaps(Offset.zero & screen) && r.width > 0 && r.height > 0) {{
          var label = labelIn(e);
          if (label == null && tn(w) == 'Icon') {{ try {{ label = '${{(w as dynamic).icon}}'; }} catch (_) {{}} }}
          out.add([tn(w), clean(label), clean(keyOf(w)), r.left.toStringAsFixed(1), r.top.toStringAsFixed(1),
            r.width.toStringAsFixed(1), r.height.toStringAsFixed(1), hit(ro, r)].join(sep));
          n++;
          return;
        }}
      }}
    }}
    e.visitChildren(visit);
  }}
  visit(root);
  return out.join(rs);
}})()"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dart_str_escapes_quotes_dollars_and_newlines() {
        assert_eq!(dart_str("it's $5\n\\"), r"'it\'s \$5\n\\'");
    }

    #[test]
    fn predicate_ands_every_field() {
        let f = Finder {
            text: Some("Save".into()),
            type_name: Some("ElevatedButton".into()),
            ..Finder::default()
        };
        let p = f.dart_predicate();
        assert!(p.contains("txt(w) == 'Save'"));
        assert!(p.contains(" && tn(w) == 'ElevatedButton'"));
        assert_eq!(Finder::default().dart_predicate(), "false");
    }

    #[test]
    fn find_expression_only_acts_with_an_index() {
        let f = Finder {
            tooltip: Some("Increment".into()),
            ..Finder::default()
        };
        let find = find_expression(&f, true, None, "return 'x';");
        assert!(!find.contains("cands[0]"));
        let act = find_expression(&f, true, Some(0), "return 'x';");
        assert!(act.contains("final el = cands[0].el"));
    }
}
