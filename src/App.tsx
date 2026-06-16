import "./App.css";

import { Component, ErrorInfo, ReactNode, Suspense, lazy } from "react";
import { HashRouter, Route, Routes } from "react-router-dom";
import { ThemeProvider } from "./components/theme-provider";
import { Toaster } from "./components/ui/sonner";
import ScreenDrawing from "./pages/screen-drawing";

const Visualization = lazy(() =>
  import("./pages/visualization").then((module) => ({ default: module.Visualization })),
);
const Settings = lazy(() => import("./pages/settings"));

class AppErrorBoundary extends Component<
  { children: ReactNode },
  { error: Error | null }
> {
  state = { error: null as Error | null };

  static getDerivedStateFromError(error: Error) {
    return { error };
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    console.error("Keyviz interface error", error, info);
  }

  render() {
    if (this.state.error) {
      return (
        <div className="m-6 rounded-lg border border-red-300 bg-red-50 p-4 text-red-900">
          <strong>Keyviz interface error</strong>
          <pre className="mt-2 whitespace-pre-wrap text-xs">{this.state.error.message}</pre>
        </div>
      );
    }
    return this.props.children;
  }
}

function App() {
  const mode = new URLSearchParams(window.location.search).get("mode");
  if (mode?.startsWith("drawing")) {
    return (
      <AppErrorBoundary>
        <ScreenDrawing />
      </AppErrorBoundary>
    );
  }

  return (
    <AppErrorBoundary>
      <HashRouter>
        <Suspense fallback={<div>Loading...</div>}>
          <Routes>
            <Route path="/" element={<Visualization />} />
            <Route path="/settings" element={
              <ThemeProvider>
                <Settings />
                <Toaster position="bottom-right" />
              </ThemeProvider>
            } />
            <Route path="/drawing" element={<ScreenDrawing />} />
          </Routes>
        </Suspense>
      </HashRouter>
    </AppErrorBoundary>
  );
}

export default App;
