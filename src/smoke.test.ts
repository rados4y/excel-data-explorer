import { expect, test } from "vitest";
import pkg from "../package.json";
test("versioned application", () => expect(pkg.version).toBe("0.1.0"));
