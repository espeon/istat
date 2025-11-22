import type {} from "@atcute/lexicons";
import * as v from "@atcute/lexicons/validations";
import type {} from "@atcute/lexicons/ambient";

const _mainSchema = /*#__PURE__*/ v.query("vg.nat.istat.moderation.isAdmin", {
  params: null,
  output: {
    type: "lex",
    schema: /*#__PURE__*/ v.object({
      /**
       * Whether the current user is an admin
       */
      isAdmin: /*#__PURE__*/ v.boolean(),
    }),
  },
});

type main$schematype = typeof _mainSchema;

export interface mainSchema extends main$schematype {}

export const mainSchema = _mainSchema as mainSchema;

export interface $params {}
export interface $output extends v.InferXRPCBodyInput<mainSchema["output"]> {}

declare module "@atcute/lexicons/ambient" {
  interface XRPCQueries {
    "vg.nat.istat.moderation.isAdmin": mainSchema;
  }
}
