import { describe, expect, it } from "vite-plus/test";

describe("frontend smoke tests", () => {
  it("creates a DOM node in the test environment", () => {
    const element = document.createElement("div");
    element.id = "smoke-test";
    document.body.appendChild(element);

    expect(document.querySelector("#smoke-test")).not.toBeNull();
  });
});
