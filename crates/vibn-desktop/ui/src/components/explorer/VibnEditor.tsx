import { useEffect, useRef } from "react";
import { EditorState, Compartment } from "@codemirror/state";
import {
  EditorView,
  keymap,
  lineNumbers,
  highlightActiveLineGutter,
  highlightActiveLine,
} from "@codemirror/view";
import { defaultKeymap, history, historyKeymap, indentWithTab } from "@codemirror/commands";
import { searchKeymap, highlightSelectionMatches } from "@codemirror/search";
import {
  bracketMatching,
  foldGutter,
  indentOnInput,
  defaultHighlightStyle,
  syntaxHighlighting,
} from "@codemirror/language";
import { oneDark } from "@codemirror/theme-one-dark";
import type { Extension } from "@codemirror/state";

interface Props {
  path: string;
  language: string;
  value: string;
  readOnly: boolean;
  onChange: (value: string) => void;
  onSave: () => void;
}

async function loadLanguage(lang: string): Promise<Extension | null> {
  try {
    switch (lang) {
      case "javascript":
        return (await import("@codemirror/lang-javascript")).javascript({ jsx: true });
      case "typescript":
        return (await import("@codemirror/lang-javascript")).javascript({
          jsx: true,
          typescript: true,
        });
      case "rust":
        return (await import("@codemirror/lang-rust")).rust();
      case "python":
        return (await import("@codemirror/lang-python")).python();
      case "json":
        return (await import("@codemirror/lang-json")).json();
      case "markdown":
        return (await import("@codemirror/lang-markdown")).markdown();
      case "html":
        return (await import("@codemirror/lang-html")).html();
      case "css":
      case "scss":
        return (await import("@codemirror/lang-css")).css();
      case "yaml":
        return (await import("@codemirror/lang-yaml")).yaml();
      case "go":
        return (await import("@codemirror/lang-go")).go();
      case "sql":
        return (await import("@codemirror/lang-sql")).sql();
      default:
        return null;
    }
  } catch (err) {
    console.warn("[vibn] language load failed", lang, err);
    return null;
  }
}

export function VibnEditor({ path, language, value, readOnly, onChange, onSave }: Props) {
  const hostRef = useRef<HTMLDivElement>(null);
  const viewRef = useRef<EditorView | null>(null);
  const onChangeRef = useRef(onChange);
  const onSaveRef = useRef(onSave);
  const langCompartment = useRef(new Compartment());
  const readOnlyCompartment = useRef(new Compartment());

  // Keep latest callbacks without forcing re-init.
  useEffect(() => {
    onChangeRef.current = onChange;
  }, [onChange]);
  useEffect(() => {
    onSaveRef.current = onSave;
  }, [onSave]);

  // Init / re-init on path change.
  useEffect(() => {
    const host = hostRef.current;
    if (!host) return;
    let cancelled = false;

    (async () => {
      const langExt = (await loadLanguage(language)) ?? [];
      if (cancelled || !hostRef.current) return;

      const extensions: Extension[] = [
        lineNumbers(),
        highlightActiveLineGutter(),
        highlightActiveLine(),
        foldGutter(),
        history(),
        indentOnInput(),
        bracketMatching(),
        syntaxHighlighting(defaultHighlightStyle, { fallback: true }),
        highlightSelectionMatches(),
        keymap.of([
          ...defaultKeymap,
          ...historyKeymap,
          ...searchKeymap,
          indentWithTab,
          {
            key: "Mod-s",
            preventDefault: true,
            run: () => {
              onSaveRef.current();
              return true;
            },
          },
        ]),
        EditorView.lineWrapping,
        oneDark,
        langCompartment.current.of(langExt),
        readOnlyCompartment.current.of([
          EditorState.readOnly.of(readOnly),
          EditorView.editable.of(!readOnly),
        ]),
        EditorView.updateListener.of((update) => {
          if (update.docChanged) {
            onChangeRef.current(update.state.doc.toString());
          }
        }),
      ];

      const state = EditorState.create({ doc: value, extensions });
      // Destroy prior view if any.
      viewRef.current?.destroy();
      const view = new EditorView({ state, parent: hostRef.current });
      viewRef.current = view;
    })();

    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [path]);

  // Push readOnly changes without re-creating the view.
  useEffect(() => {
    const view = viewRef.current;
    if (!view) return;
    view.dispatch({
      effects: readOnlyCompartment.current.reconfigure([
        EditorState.readOnly.of(readOnly),
        EditorView.editable.of(!readOnly),
      ]),
    });
  }, [readOnly]);

  // Push external value updates (e.g. agent edited the file underneath).
  useEffect(() => {
    const view = viewRef.current;
    if (!view) return;
    if (view.state.doc.toString() === value) return;
    view.dispatch({ changes: { from: 0, to: view.state.doc.length, insert: value } });
  }, [value]);

  // Tear down on unmount.
  useEffect(() => {
    return () => {
      viewRef.current?.destroy();
      viewRef.current = null;
    };
  }, []);

  return <div ref={hostRef} className="h-full w-full overflow-auto bg-zinc-950 text-[13px]" />;
}
