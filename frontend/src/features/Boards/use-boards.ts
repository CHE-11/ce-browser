import { useEffect, useState } from "react";
import { listBoards, onBoardsChanged } from "@/lib/browser-client";
import type { BoardsSnapshot } from "@type/browser";
import { EMPTY_BOARDS } from "./board-tree";

/** Folders, boards and pins, kept current through `boards-changed`. */
export function useBoards() {
  const [boards, setBoards] = useState<BoardsSnapshot>(EMPTY_BOARDS);
  const [loaded, setLoaded] = useState(false);

  useEffect(() => {
    let gone = false;
    const unlisten = onBoardsChanged((next) => {
      if (!gone) {
        setBoards(next);
        setLoaded(true);
      }
    });
    void listBoards()
      .then((next) => {
        if (!gone) {
          setBoards(next);
        }
      })
      .catch(() => undefined)
      .finally(() => {
        if (!gone) {
          setLoaded(true);
        }
      });
    return () => {
      gone = true;
      void unlisten.then((stop) => stop());
    };
  }, []);

  return { boards, loaded };
}
