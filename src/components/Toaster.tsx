import React from "react";
import { Toaster } from "react-hot-toast";

export function CustomToaster(): React.JSX.Element {
  return (
    <Toaster
      position="bottom-right"
      toastOptions={{
        style: {
          background: "rgba(18, 18, 18, 0.5)",
          backdropFilter: "blur(12px)",
          WebkitBackdropFilter: "blur(12px)",
          border: "1px solid rgba(255, 255, 255, 0.1)",
          borderRadius: "12px",
          boxShadow: "0 4px 20px rgba(0, 0, 0, 0.25)",
          color: "#fff",
          padding: "14px 20px",
          fontFamily:
            "-apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif",
        },
        success: {
          iconTheme: {
            primary: "#30D158",
            secondary: "#fff",
          },
        },
        error: {
          iconTheme: {
            primary: "#FF375F",
            secondary: "#fff",
          },
        },
      }}
    />
  );
}
