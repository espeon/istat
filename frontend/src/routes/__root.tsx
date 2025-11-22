import { Outlet, createRootRoute } from "@tanstack/react-router";
import { ToastProvider } from "../lib/toast";
// import { TanStackRouterDevtoolsPanel } from '@tanstack/react-router-devtools'
// import { TanStackDevtools } from '@tanstack/react-devtools'

export const Route = createRootRoute({
  component: () => (
    <ToastProvider>
      <Outlet />
      {/*<TanStackDevtools
        config={{
          position: 'bottom-right',
        }}
        plugins={[
          {
            name: 'Tanstack Router',
            render: <TanStackRouterDevtoolsPanel />,
          },
        ]}
      />*/}
    </ToastProvider>
  ),
});
