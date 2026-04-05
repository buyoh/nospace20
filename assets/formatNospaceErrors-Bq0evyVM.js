function o(t){return t.map(r=>{const e=r.line!=null?`:${r.line}${r.column!=null?":"+r.column:""}`:"",s=r.details?`
  ${r.details}`:"";return`${r.message}${e}${s}`}).join(`
`)}function l(t){let r;try{r=JSON.parse(t)}catch{return null}return n(r)?o(r.errors):null}function n(t){if(typeof t!="object"||t===null)return!1;const r=t;return r.success!==!1||!Array.isArray(r.errors)?!1:r.errors.every(e=>typeof e=="object"&&e!==null&&typeof e.message=="string")}function a(t){let r;try{r=JSON.parse(t)}catch{return null}return n(r)?r.errors:null}export{a,o as f,n as i,l as t};
