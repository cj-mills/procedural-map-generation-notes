//! The published JSON Schema for recipes, written from the core's `STAGES` table and `docs`.

use crate::docs;
use map_lab_core::recipe::{push_decimal, Param, FORMAT, HEIGHT, MAX_SEED, MAX_SIDE, MIN_SIDE, STAGES, WIDTH};
use map_lab_core::report::{list, string, Obj};

/// A parameter's decimal as the recipe writes it: `0.1`, not the `f32`'s digits as an `f64`.
fn decimal(x: f32) -> String {
    let mut s = String::new();
    push_decimal(&mut s, x);
    s
}

pub fn schema() -> String {
    let mut props = Obj::lines(4);
    let mut f = Obj::new();
    f.raw("const", &string(FORMAT)).raw("description", &string("The recipe format and its version."));
    props.raw("format", &f.end());
    for k in ["title", "note"] {
        let mut o = Obj::new();
        o.raw("type", &string("string")).raw("description", &string("Free text for people; ignored by the build."));
        props.raw(k, &o.end());
    }
    let mut seed = Obj::new();
    seed.raw("type", &string("integer")).raw("minimum", "0").raw("maximum", &MAX_SEED.to_string());
    seed.raw("description", &string("Seeds the one random number generator every stage draws from, in pipeline order. Kept within 2^53 - 1 so it survives JavaScript numbers."));
    props.raw("seed", &seed.end());
    for (k, d) in [("width", WIDTH), ("height", HEIGHT)] {
        let mut o = Obj::new();
        o.raw("type", &string("integer")).raw("minimum", &MIN_SIDE.to_string()).raw("maximum", &MAX_SIDE.to_string());
        o.raw("default", &d.to_string()).raw("description", &string("Map size in tiles; the talk's maps are 80 by 50."));
        props.raw(k, &o.end());
    }
    let mut p = Obj::new();
    p.raw("$ref", &string("#/$defs/pipeline"));
    props.raw("pipeline", &p.end());

    let mut defs = Obj::lines(4);
    let mut pipe = Obj::lines(6);
    pipe.raw("type", &string("array")).raw("minItems", "1");
    pipe.raw(
        "description",
        &string("Stages run in order on one map. A generator starts a new map and forgets what was placed on the old one. Order rules the parser enforces: exit needs a start before it; hot-path needs an exit; hot-rooms needs a rooms or bsp generator and an exit; prefab with place=room needs a rooms or bsp generator; dla-erode needs a stage before it."),
    );
    let refs = STAGES.iter().map(|s| {
        let mut o = Obj::new();
        o.raw("$ref", &string(&format!("#/$defs/stage-{}", s.name)));
        o.end()
    });
    let mut items = Obj::new();
    items.raw("oneOf", &list(refs));
    pipe.raw("items", &items.end());
    defs.raw("pipeline", &pipe.end());

    for s in STAGES {
        let (_, what, demo) = docs::STAGES.iter().find(|d| d.0 == s.name).expect("docs for every stage");
        let mut st = Obj::lines(6);
        let mut required = vec![string("stage")];
        required.extend(s.params.iter().filter(|p| matches!(p.kind, Param::Inputs(_))).map(|p| string(p.name)));
        st.raw("type", &string("object")).raw("required", &list(required));
        st.raw("additionalProperties", "false").raw("description", &string(what));
        st.raw("x-talk-demo", &string(demo));
        let mut ps = Obj::lines(8);
        let mut name = Obj::new();
        name.raw("const", &string(s.name));
        ps.raw("stage", &name.end());
        let mut v = Obj::new();
        v.raw("enum", &list(s.variants.iter().map(|x| string(x)))).raw("default", &string(s.variants[0]));
        v.raw("description", &string("Which version of the technique: \"talk\" is the talk's own. Labeled variants, each with its own source and a measured difference, come after it."));
        ps.raw("variant", &v.end());
        for prm in s.params {
            let (_, _, d) = docs::PARAMS.iter().find(|d| d.0 == s.name && d.1 == prm.name).expect("docs for every parameter");
            let mut o = Obj::new();
            match &prm.kind {
                Param::Int { min, max, default } => {
                    o.raw("type", &string("integer")).raw("minimum", &min.to_string()).raw("maximum", &max.to_string()).raw("default", &default.to_string());
                }
                Param::Num { min, max, default } => {
                    o.raw("type", &string("number")).raw("minimum", &decimal(*min)).raw("maximum", &decimal(*max));
                    o.raw("default", &decimal(*default));
                }
                Param::Pick(options) => {
                    o.raw("enum", &list(options.iter().map(|x| string(x)))).raw("default", &string(options[0]));
                }
                Param::Inputs(n) => {
                    let mut it = Obj::new();
                    it.raw("$ref", &string("#/$defs/pipeline"));
                    o.raw("type", &string("array")).raw("minItems", &n.to_string()).raw("maxItems", &n.to_string()).raw("items", &it.end());
                }
            }
            o.raw("description", &string(d));
            ps.raw(prm.name, &o.end());
        }
        st.raw("properties", &ps.end());
        defs.raw(&format!("stage-{}", s.name), &st.end());
    }

    let mut o = Obj::lines(2);
    o.raw("$schema", &string("https://json-schema.org/draft/2020-12/schema"));
    o.raw("$id", &string("https://github.com/cj-mills/procedural-map-generation-notes/blob/main/lab/schema/recipe.schema.json"));
    o.raw("title", &string("Map lab recipe"));
    o.raw("description", &string(docs::RECIPE));
    o.raw("type", &string("object"));
    o.raw("required", &list(["format", "seed", "pipeline"].map(string)));
    o.raw("additionalProperties", "false");
    o.raw("properties", &props.end());
    o.raw("$defs", &defs.end());
    o.end()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn docs_cover_the_core_exactly() {
        for s in STAGES {
            assert!(docs::STAGES.iter().any(|d| d.0 == s.name), "no docs for stage {}", s.name);
            for p in s.params {
                assert!(docs::PARAMS.iter().any(|d| d.0 == s.name && d.1 == p.name), "no docs for {}.{}", s.name, p.name);
            }
        }
        assert_eq!(docs::STAGES.len(), STAGES.len());
        assert_eq!(docs::PARAMS.len(), STAGES.iter().map(|s| s.params.len()).sum::<usize>());
    }

    #[test]
    fn committed_schema_is_current() {
        let committed = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../schema/recipe.schema.json")).unwrap();
        assert_eq!(committed.trim_end(), schema(), "run: map-lab schema > lab/schema/recipe.schema.json");
    }

    #[test]
    fn schema_is_json_the_core_reads() {
        assert!(map_lab_core::json::parse(&schema()).is_ok());
    }
}
