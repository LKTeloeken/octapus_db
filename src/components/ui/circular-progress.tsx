"use client";

import * as React from "react";
import { cn } from "@/lib/utils";

interface CircularProgressProps extends React.HTMLAttributes<HTMLDivElement> {
  /** Progress value between 0 and 100 */
  value?: number;
  /** Size of the circular progress (width and height) */
  size?: number | string;
  /** Thickness of the stroke */
  thickness?: number;
}

const CircularProgress = React.forwardRef<
  HTMLDivElement,
  CircularProgressProps
>(({ value, size = 40, thickness = 4, className, ...props }, ref) => {
  const numericSize = Number(size);
  const radius = (numericSize - thickness) / 2;
  const circumference = 2 * Math.PI * radius;
  const offset = circumference - ((value ?? 0) / 100) * circumference;
  const isDeterminate = value != null;

  return (
    <div
      ref={ref}
      role="progressbar"
      aria-valuemin={0}
      aria-valuemax={100}
      aria-valuenow={value ?? undefined}
      className={cn("inline-block", className)}
      style={{ width: size, height: size }}
      {...props}
    >
      {isDeterminate ? (
        <svg
          className="transform -rotate-90"
          width={numericSize}
          height={numericSize}
        >
          {/* Track */}
          <circle
            className="text-muted-foreground"
            strokeWidth={thickness}
            stroke="currentColor"
            fill="transparent"
            r={radius}
            cx={numericSize / 2}
            cy={numericSize / 2}
            style={{ strokeDasharray: circumference }}
          />
          {/* Indicator */}
          <circle
            className="text-primary"
            strokeWidth={thickness}
            stroke="currentColor"
            fill="transparent"
            r={radius}
            cx={numericSize / 2}
            cy={numericSize / 2}
            style={{
              strokeDasharray: circumference,
              strokeDashoffset: offset,
              transition: "stroke-dashoffset 0.35s",
            }}
          />
        </svg>
      ) : (
        <svg
          className="animate-spin transform -rotate-90"
          width={numericSize}
          height={numericSize}
        >
          <circle
            className="opacity-25 text-muted-foreground"
            strokeWidth={thickness}
            stroke="currentColor"
            fill="transparent"
            r={radius}
            cx={numericSize / 2}
            cy={numericSize / 2}
          />
          <circle
            className="opacity-75 text-primary"
            strokeWidth={thickness}
            stroke="currentColor"
            fill="transparent"
            r={radius}
            cx={numericSize / 2}
            cy={numericSize / 2}
            style={{
              strokeDasharray: circumference * 0.75,
              strokeDashoffset: circumference * 0.25,
            }}
          />
        </svg>
      )}
    </div>
  );
});
CircularProgress.displayName = "CircularProgress";

export { CircularProgress };
