import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import "./index.css";
import { SearchPage } from "./pages/search/index.tsx";
import { createBrowserRouter, RouterProvider } from "react-router-dom";
import ErrorPage from "./error-page.tsx";
import { WizardPage } from "./pages/wizard/WizardPage.tsx";

const router = createBrowserRouter([
  {
    path: "/",
    element: <SearchPage />,
    errorElement: <ErrorPage />,
  },
  {
    path: "/progress",
    element: <div>Progress Popup</div>,
  },
  {
    path: "/settings:tab",
    element: <div>settings tab</div>,
  },
  {
    path: "/startup",
    element: <div>startup</div>,
  },
  {
    path: "/updater",
    element: <div>updater</div>,
  },
  {
    path: "/wizard",
    element: <WizardPage />,
  },
]);

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <RouterProvider router={router} />
  </StrictMode>,
);
