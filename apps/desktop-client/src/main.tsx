import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import "./index.css";
import { SearchPage } from "./pages/search/SearchPage.tsx";
import { createBrowserRouter, RouterProvider } from "react-router-dom";
import ErrorPage from "./error-page.tsx";
import { WizardPage } from "./pages/wizard/WizardPage.tsx";
import { SettingsPage } from "./pages/settings/SettingsPage.tsx";
import { ProgressPopup } from "./pages/ProgressPopup.tsx";
import { BigMode } from "./pages/bigmode/BigMode.tsx";
import { StartupPopup } from "./pages/StartupPopup.tsx";

const router = createBrowserRouter([
  {
    path: "/",
    element: <SearchPage />,
    errorElement: <ErrorPage />,
  },
  {
    path: "/bigmode",
    element: <BigMode />,
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
    element: <StartupPopup />,
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
