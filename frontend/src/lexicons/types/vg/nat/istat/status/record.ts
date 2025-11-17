import type {} from "@atcute/lexicons";
import * as v from "@atcute/lexicons/validations";
import type {} from "@atcute/lexicons/ambient";
import * as ComAtprotoRepoStrongRef from "@atcute/client/lexicons";

const _mainSchema = /*#__PURE__*/ v.record(
  /*#__PURE__*/ v.tidString(),
  /*#__PURE__*/ v.object({
    $type: /*#__PURE__*/ v.literal("vg.nat.istat.status.record"),
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
     * The emoji representing the status
     */
    get emoji() {
      return ComAtprotoRepoStrongRef.mainSchema;
    },
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
);

type main$schematype = typeof _mainSchema;

export interface mainSchema extends main$schematype {}

export const mainSchema = _mainSchema as mainSchema;

export interface Main extends v.InferInput<typeof mainSchema> {}

declare module "@atcute/lexicons/ambient" {
  interface Records {
    "vg.nat.istat.status.record": mainSchema;
  }
}
