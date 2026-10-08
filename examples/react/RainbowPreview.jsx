import { useEffect, useState } from "react";
import init, { parse, validate } from "rainbow-parser-wasm-web";

let ready = null;

function loadParser() {
  ready ??= init();
  return ready;
}

export function RainbowPreview({ source }) {
  const [state, setState] = useState(null);

  useEffect(() => {
    let cancelled = false;
    loadParser().then(() => {
      if (cancelled) return;
      setState({
        parsed: JSON.parse(parse(source)),
        validated: JSON.parse(validate(source, false)),
      });
    });
    return () => {
      cancelled = true;
    };
  }, [source]);

  if (!state) return null;
  return <pre>{JSON.stringify(state, null, 2)}</pre>;
}
