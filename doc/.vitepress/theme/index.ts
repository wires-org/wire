import DefaultTheme from "vitepress/theme";
import "virtual:group-icons.css";
import giscusTalk from "vitepress-plugin-comment-with-giscus";
import { EnhanceAppContext, useData, useRoute } from "vitepress";
import { toRefs } from "vue";

export default {
  ...DefaultTheme,
  enhanceApp(ctx: EnhanceAppContext) {
    DefaultTheme.enhanceApp(ctx);
  },
  setup() {
    const { frontmatter } = toRefs(useData());
    const route = useRoute();

    giscusTalk(
      {
        repo: "wires-org/wire",
        repoId: "R_kgDOMQQbzw",
        category: "giscus", // default: `General`
        categoryId: "DIC_kwDOMQQbz84Co4vv",
        mapping: "pathname",
        inputPosition: "top",
        lang: "en",
        // i18n setting (Note: This configuration will override the default language set by lang)
        // Configured as an object with key-value pairs inside:
        // [your i18n configuration name]: [corresponds to the language pack name in Giscus]
        locales: {
          "en-US": "en",
        },
        homePageShowComment: false,
        lightTheme: "light",
        darkTheme: "transparent_dark",
      },
      {
        frontmatter,
        route,
      },
      // Default to false for all pages
      false,
    );
  },
};
