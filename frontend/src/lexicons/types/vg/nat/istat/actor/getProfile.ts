import type {} from "@atcute/lexicons";
import * as v from "@atcute/lexicons/validations";
import type {} from "@atcute/lexicons/ambient";

const _mainSchema = /*#__PURE__*/ v.query("vg.nat.istat.actor.getProfile", {
  params: /*#__PURE__*/ v.object({
    /**
     * Handle or DID of account to fetch profile of
     */
    actor: /*#__PURE__*/ v.actorIdentifierString(),
  }),
  output: {
    type: "lex",
    schema: /*#__PURE__*/ v.object({
      /**
       * URL to avatar image
       */
      avatar: /*#__PURE__*/ v.optional(/*#__PURE__*/ v.string()),
      /**
       * URL to banner image
       */
      banner: /*#__PURE__*/ v.optional(/*#__PURE__*/ v.string()),
      createdAt: /*#__PURE__*/ v.optional(/*#__PURE__*/ v.datetimeString()),
      /**
       * @maxLength 2560
       * @maxGraphemes 256
       */
      description: /*#__PURE__*/ v.optional(
        /*#__PURE__*/ v.constrain(/*#__PURE__*/ v.string(), [
          /*#__PURE__*/ v.stringLength(0, 2560),
          /*#__PURE__*/ v.stringGraphemes(0, 256),
        ]),
      ),
      did: /*#__PURE__*/ v.didString(),
      /**
       * @maxLength 640
       * @maxGraphemes 64
       */
      displayName: /*#__PURE__*/ v.optional(
        /*#__PURE__*/ v.constrain(/*#__PURE__*/ v.string(), [
          /*#__PURE__*/ v.stringLength(0, 640),
          /*#__PURE__*/ v.stringGraphemes(0, 64),
        ]),
      ),
      handle: /*#__PURE__*/ v.handleString(),
      /**
       * @maxLength 64
       */
      pronouns: /*#__PURE__*/ v.optional(
        /*#__PURE__*/ v.constrain(/*#__PURE__*/ v.string(), [
          /*#__PURE__*/ v.stringLength(0, 64),
        ]),
      ),
      /**
       * @maxLength 512
       */
      website: /*#__PURE__*/ v.optional(
        /*#__PURE__*/ v.constrain(/*#__PURE__*/ v.string(), [
          /*#__PURE__*/ v.stringLength(0, 512),
        ]),
      ),
    }),
  },
});

type main$schematype = typeof _mainSchema;

export interface mainSchema extends main$schematype {}

export const mainSchema = _mainSchema as mainSchema;

export interface $params extends v.InferInput<mainSchema["params"]> {}
export interface $output extends v.InferXRPCBodyInput<mainSchema["output"]> {}

declare module "@atcute/lexicons/ambient" {
  interface XRPCQueries {
    "vg.nat.istat.actor.getProfile": mainSchema;
  }
}
