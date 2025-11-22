import type {} from "@atcute/lexicons";
import * as v from "@atcute/lexicons/validations";
import type {} from "@atcute/lexicons/ambient";

const _mainSchema = /*#__PURE__*/ v.procedure(
  "vg.nat.istat.moderation.blacklistCid",
  {
    params: null,
    input: {
      type: "lex",
      schema: /*#__PURE__*/ v.object({
        /**
         * The CID to blacklist
         */
        cid: /*#__PURE__*/ v.string(),
        /**
         * Type of content being blacklisted
         */
        contentType: /*#__PURE__*/ v.literalEnum([
          "avatar",
          "banner",
          "emoji_blob",
        ]),
        /**
         * Predefined reason for blacklisting
         */
        reason: /*#__PURE__*/ v.literalEnum([
          "copyright",
          "gore",
          "harassment",
          "nudity",
          "other",
          "spam",
        ]),
        /**
         * Additional details explaining the blacklist reason
         * @maxLength 1000
         */
        reasonDetails: /*#__PURE__*/ v.optional(
          /*#__PURE__*/ v.constrain(/*#__PURE__*/ v.string(), [
            /*#__PURE__*/ v.stringLength(0, 1000),
          ]),
        ),
      }),
    },
    output: {
      type: "lex",
      schema: /*#__PURE__*/ v.object({
        success: /*#__PURE__*/ v.boolean(),
      }),
    },
  },
);

type main$schematype = typeof _mainSchema;

export interface mainSchema extends main$schematype {}

export const mainSchema = _mainSchema as mainSchema;

export interface $params {}
export interface $input extends v.InferXRPCBodyInput<mainSchema["input"]> {}
export interface $output extends v.InferXRPCBodyInput<mainSchema["output"]> {}

declare module "@atcute/lexicons/ambient" {
  interface XRPCProcedures {
    "vg.nat.istat.moderation.blacklistCid": mainSchema;
  }
}
