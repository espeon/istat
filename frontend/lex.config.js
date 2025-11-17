import { defineLexiconConfig } from "@atcute/lex-cli";

export default defineLexiconConfig({
  files: ["../lex/**/*.json"],
  outdir: "./src/lexicons/",
  mappings: [
    {
      nsid: ["com.atproto.*"],
      imports: () => ({
        type: "namespace",
        from: "@atcute/client/lexicons",
      }),
    },
  ],
});
