"use client";

// @awa-component: DEV-ResizeHandle

import { useCallback, useEffect, useState } from "react";
import { MIN_PANEL_WIDTH, MIN_PANEL_HEIGHT, MAX_PANEL_WIDTH_RATIO, MAX_PANEL_HEIGHT_RATIO } from "@/lib/types";

interface ResizeHandleProps {
  orientation: "horizontal" | "vertical";
  onResize: (size: number) => void;
  currentSize: number;
  onDragStateChange?: (isDragging: boolean) => void;
}

// @awa-impl: DEV-1_AC-8
export function ResizeHandle({ orientation, onResize, currentSize, onDragStateChange }: ResizeHandleProps) {
  const [isDragging, setIsDragging] = useState(false);
  const [startPos, setStartPos] = useState(0);
  const [startSize, setStartSize] = useState(0);

  // Notify parent of drag state changes
  useEffect(() => {
    onDragStateChange?.(isDragging);
  }, [isDragging, onDragStateChange]);

  const handleMouseDown = useCallback(
    (e: React.MouseEvent) => {
      e.preventDefault();
      setIsDragging(true);
      setStartPos(orientation === "horizontal" ? e.clientX : e.clientY);
      setStartSize(currentSize);
    },
    [orientation, currentSize],
  );

  useEffect(() => {
    if (!isDragging) return;

    const handleMouseMove = (e: MouseEvent) => {
      const currentPos = orientation === "horizontal" ? e.clientX : e.clientY;
      // For horizontal (width), dragging left increases size (panel is on right)
      // For vertical (height), dragging up increases size (panel is at bottom)
      const delta = startPos - currentPos;
      const newSize = startSize + delta;

      // Apply constraints - max is 2/3 of screen size
      const min = orientation === "horizontal" ? MIN_PANEL_WIDTH : MIN_PANEL_HEIGHT;
      const max = orientation === "horizontal" ? window.innerWidth * MAX_PANEL_WIDTH_RATIO : window.innerHeight * MAX_PANEL_HEIGHT_RATIO;
      const constrainedSize = Math.min(Math.max(newSize, min), max);

      onResize(constrainedSize);
    };

    const handleMouseUp = () => {
      setIsDragging(false);
    };

    document.addEventListener("mousemove", handleMouseMove);
    document.addEventListener("mouseup", handleMouseUp);

    return () => {
      document.removeEventListener("mousemove", handleMouseMove);
      document.removeEventListener("mouseup", handleMouseUp);
    };
  }, [isDragging, orientation, startPos, startSize, onResize]);

  const isHorizontal = orientation === "horizontal";

  return <div onMouseDown={handleMouseDown} className={`absolute ${isHorizontal ? "left-0 top-0 bottom-0 w-1 cursor-col-resize" : "top-0 left-0 right-0 h-1 cursor-row-resize"} hover:bg-blue-500 transition-colors z-20 ${isDragging ? "bg-blue-500" : "bg-transparent"}`} aria-label="Resize panel" />;
}
