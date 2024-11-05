import { ErrorResponse, useRouteError } from "react-router-dom";

export default function ErrorPage() {
  const error = useRouteError() as Error | ErrorResponse;
  console.error(error);
  return (
    <div id="error-page">
      <h1>Oops!</h1>
      <p>Sorry, an unexpected error has occurred.</p>
      <p>
        <i>{"statusText" in error ? error.statusText : error.message}</i>
      </p>
    </div>
  );
}
