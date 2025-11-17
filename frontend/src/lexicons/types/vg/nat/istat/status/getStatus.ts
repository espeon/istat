import type {} from "@atcute/lexicons";
import * as v from "@atcute/lexicons/validations";
import type {} from "@atcute/lexicons/ambient";

const _mainSchema = /*#__PURE__*/ v.query("vg.nat.istat.status.getStatus", {
  params: /*#__PURE__*/ v.object({
    /**
     * The handle of the user whose status to retrieve
     */
    handle: /*#__PURE__*/ v.handleString(),
    /**
     * The record key (tid) of the status
     */
    rkey: /*#__PURE__*/ v.string(),
  }),
  output: {
    type: "lex",
    schema: /*#__PURE__*/ v.object({
      /**
       * When this status was created
       */
      createdAt: /*#__PURE__*/ v.datetimeString(),
      /**
       * Optional status text description
       * @maxLength 20480
       * @maxGraphemes 2048
       */
      description: /*#__PURE__*/ v.optional(
        /*#__PURE__*/ v.constrain(/*#__PURE__*/ v.string(), [
          /*#__PURE__*/ v.stringLength(0, 20480),
          /*#__PURE__*/ v.stringGraphemes(0, 2048),
        ]),
      ),
      /**
       * a URL to the emoji
       */
      emojiUrl: /*#__PURE__*/ v.string(),
      /**
       * Optional expiration timestamp for this status
       */
      expires: /*#__PURE__*/ v.optional(/*#__PURE__*/ v.datetimeString()),
      /**
       * Optional status text title
       * @maxLength 2560
       * @maxGraphemes 256
       */
      title: /*#__PURE__*/ v.optional(
        /*#__PURE__*/ v.constrain(/*#__PURE__*/ v.string(), [
          /*#__PURE__*/ v.stringLength(0, 2560),
          /*#__PURE__*/ v.stringGraphemes(0, 256),
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
    "vg.nat.istat.status.getStatus": mainSchema;
  }
}
