import { createRoot } from "react-dom/client";
import { StrictMode, createElement } from "react";
import App from "@/App";

// Get the root element
const container = document.getElementById("root");

// Ensure the container exists
if (!container) {
  throw new Error("Root element not found in the DOM");
}

// Create root using the new React 19 API
const root = createRoot(container);

// Render the application
root.render(createElement(StrictMode, null, createElement(App, null)));
