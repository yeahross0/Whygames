register_plugin = function (importObject) {
    importObject.env.hi_from_wasm = function (js_object) {
        console.log("hi")
        let s = consume_js_object(js_object);
        var element = document.createElement('a');
        element.setAttribute('href', 'data:image/png;base64,' + s);
        element.setAttribute('download', "out" + Math.floor(Math.random() * 99999) + ".png");

        console.log("hi2")

        element.style.display = 'none';
        document.body.appendChild(element);

        element.click();

        document.body.removeChild(element);

        console.log("hi3")

    }
}

document.onclick = function () {
    // and rust from JS!
    //wasm_exports.hi_from_rust();
};

// miniquad_add_plugin receive an object with two fields: register_plugin and on_init. Both are functions, both are optional.
miniquad_add_plugin({ register_plugin });