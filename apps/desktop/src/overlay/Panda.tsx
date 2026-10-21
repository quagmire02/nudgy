import { useMemo } from "react";
import type { CharacterState } from "@nudgy/shared";
import pandaSvg from "../../../../character/panda.svg?raw";
import "./panda.css";

/** The armored panda, from the shared concept art, in one animation state. */
export function Panda({ state, talking }: { state: CharacterState; talking: boolean }) {
  // The SVG is static; only the classes on its root change.
  const markup = useMemo(
    () => pandaSvg.replace('class="panda"', `class="panda s-${state}${talking ? " talking" : ""}"`),
    [state, talking],
  );
  return <div className="panda-svg" dangerouslySetInnerHTML={{ __html: markup }} />;
}
