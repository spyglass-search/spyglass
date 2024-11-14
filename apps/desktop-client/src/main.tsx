import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import "./index.css";
import { SearchPage } from "./pages/search/index.tsx";
import { createBrowserRouter, RouterProvider } from "react-router-dom";
import ErrorPage from "./error-page.tsx";
import { WizardPage } from "./pages/wizard/WizardPage.tsx";
import { SettingsPage } from "./pages/settings/SettingsPage.tsx";
import { ProgressPopup } from "./pages/ProgressPopup.tsx";

const router = createBrowserRouter([
  {
    path: "/",
    element: <SearchPage />,
    errorElement: <ErrorPage />,
  },
  {
    path: "/progress",
    element: <ProgressPopup />,
  },
  {
    path: "/settings/:tab",
    loader: (params) => params,
    element: <SettingsPage />,
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
