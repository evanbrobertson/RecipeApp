import QtQuick

import app.crumb.desktop 1.0

// Api calls with callbacks, for one page:
//   requests.call("recipe", {id: 7}, recipe => …, error => …, progressLine => …)
// Answers arrive parsed (`JSON.parse`). An error callback gets the message, then
// `{code, site}`: the server's reason when it gives one (`site_blocked`, `site_terms`) and,
// with `site_terms`, the listed site's name. A call with no error callback raises `failed`. When
// the page goes, so do its pending answers.
QtObject {
    id: requests

    property var handlers: ({})
    // How many of this page's calls are still out.
    property int busy: 0

    signal failed(string error)

    function call(op, args, ok, fail, progress) {
        var id = Api.call(op, JSON.stringify(args || {}))
        handlers[id] = { "ok": ok, "fail": fail, "progress": progress }
        busy += 1
        return id
    }

    property Connections _api: Connections {
        target: Api

        function onReplied(id, ok, json, error, reason) {
            var handler = requests.handlers[id]
            if (!handler)
                return
            delete requests.handlers[id]
            requests.busy -= 1
            if (ok) {
                if (handler.ok)
                    handler.ok(json.length ? JSON.parse(json) : null)
            } else if (handler.fail) {
                var why = reason ? JSON.parse(reason) : {}
                handler.fail(error, { "code": why.code || "", "site": why.site || "" })
            } else {
                requests.failed(error)
            }
        }

        function onProgress(id, text) {
            var handler = requests.handlers[id]
            if (handler && handler.progress)
                handler.progress(text)
        }
    }
}
