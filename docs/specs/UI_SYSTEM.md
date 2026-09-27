> **Status:** proposed — UI System v2 planning revision, 2026-09-27, reconciled against `601fd7c`. Not implementation evidence or independent approval; accepted ADRs and the charter remain authoritative.
# Varos UI System v2

## المقصود من الدفعة

نبدأ بالمكونات اللي شاشة البداية محتاجاها فعلًا، ونوصل فتح الملفات والاسترجاع قبل إعادة بناء البانلات. نفس بنية البرنامج الحالية، ومسار واحد للأوامر، وصاحب واحد لكل تعديل في المستند. دفعة التخطيط دي لا تغيّر شكل البرنامج أو سلوكه.

ترتيب التنفيذ في [PLAN](../PLAN.md) فقط: S5-C/D/E → الحد الأدنى U0 → E2 → F1/F2 → S4/S6، وبعدهم باقي الواجهة على أجزاء. S5 لا ينتظر الواجهة. U1–U6 حدود مستقبلية، وليست شرطًا لشاشة البداية أو وعدًا بزمن تنفيذ.

هذه النسخة تستبدل الخطة التشغيلية في [v1 المحفوظة](../history/UI_SYSTEM_V1_THROUGH_2026-09-27.md). [جدول التصرفات](../foundation/work_orders/reviews/UI_SYSTEM.v2.disposition.md) يربط كل finding بالرد وصاحب التنفيذ. «عولج في التصميم» لا يعني «اتصلح في الكود» أو موافقة مستقلة على النسخة الجديدة.

## القرارات السارية

مصدر قرارات المالك: [سجل 24/25 سبتمبر](../history/STATUS_THROUGH_2026-09-26.md). لا يعاد طلبها أو تغيير ADR مقبول ضمن هذه الخطة.

| محسوم | أثره |
|---|---|
| Mac-first؛ Home بدل burger بجوار التبويبات؛ native menus باقية | E2 يضيف Home؛ الأيقونات تحمل أسماء واضحة للمساعدة وإتاحة الوصول. |
| تضمين IBM Plex Sans/Mono/Arabic | U0-A يتحقق من الرخصة والتغطية ويجرب أسماء عربية ومختلطة. |
| إزالة glide/ease/azure glow؛ direction bar باقٍ | U4-M يحتاج ADR superseding لـ0006 قبل تعديل fork؛ رقم 0008 مستخدم بالفعل. |
| on states مختلفة | tool = azure block؛ icon toggle = شريط azure صغير بلا fill؛ tab/segment = surface fill بلا azure. |
| FAINT→MUTED ومقاسات QW6 | لا عودة لمقترحات 9.5/16؛ المرجع المقبول micro 10.5/icon 18، والتباين على الخلفية الفعلية. |
| layout persistence مؤجل؛ Reset Workspace في Window فقط | لا ملف layout/workspace جديد الآن؛ Reset لا يمس المستندات أو التبويبات أو نافذة النظام. |
| v1 broken mask: release + notice؛ تصدير اسم .pdf: `<name> export.pdf` | يملكه S5/S6، بلا قارئ أو export مكرر داخل UI. |

Tab وcommit-before-save وsmart zoom والتفاصيل غير المسجلة تبقى مقترحات تحتاج قرارًا عند القطعة المعنية. لا تعطل E2، ولا تُعتمد ضمنيًا من الموافقة على ترتيب الخطة.

## K1 — حدود المكتبة والأوامر

`lib.rs` يصدّر shell/start/storage؛ AppCommand وSessionId وhost/workspace/ui في binary. لذلك kit في `shell/kit` مستقل عن أوامر التطبيق: يأخذ widget key، label/icon، state، enablement وسبب التعطيل، ويعيد Response أو حدث قيمة. لا يستورد CommandId/Request/HomeId من binary، ولا ينفذ I/O أو يعدّل Editor؛ help نص محلول من المستدعي.

E2 يترجم StartAction مرة واحدة في binary إلى AppCommand/lifecycle الحالي. Open/Recent/Locate تستخدم نفس الفتح. `start_ui.rs` يملك الرسم؛ `start.rs` يبقى نموذجًا بلا egui أو I/O. نضيف variants عند الحاجة ونحفظ ToggleDock وS1 signatures. لا dispatcher ثانٍ ولا نقل S1 كله إلى lib.

لاحقًا U1-A يعرّف CommandId/Request/CmdCtx قبل المستهلكين. Request له Edit/App فقط؛ CmdCtx قيمة مملوكة صغيرة مشتقة من workspace والفوكس وStart/modal/popup وselection/history/transaction/platform، لا استعارة من Snap. resolver يعمل خارج frame ومن native menu. أوامر ذات path/parameters تمر بالمسار الحالي ولا تُحشر في جدول مفاتيح ثابت. transient setters تتبع عقد F4 الحالي حتى تعديل ruling صراحة في F4_DESIGN؛ لا تعتبر هذه الخطة التعديل مقبولًا بالفعل.

## K2 — الفوكس ومسار التنفيذ

نمد `host::ActionQueue` الحالي: FIFO واحد وdrain في AboutToWait. DocAction inline مشروط بخلو الطابور وعدم pointer مؤجل. بعض pointer/panel mutations لا تزال مباشرة؛ نقلها تدريجي، فلا ندعي أن كل التعديلات queued اليوم.

| السياق | صاحب الحدث والعقد المستهدف |
|---|---|
| text field | Copy/Cut/Paste/Undo/Redo/Select All/Delete لمحرر النص أولًا، keyboard أو native menu؛ لا تعدل الرسم خلفه. File/Window تستخدم K3. |
| modal/popup | Escape/Enter والتنقل لصاحب النافذة أولًا؛ modal يمنع أوامر المستند غير المسموحة عند التنفيذ. |
| Start | StartModel الحالي: Tab/Shift-Tab، أسهم داخل القوائم، Enter وDelete لـRecent؛ لا اختصارات رسم أو Escape جديد. |
| canvas | S1 باقٍ حتى U5؛ اقتراح Tab بلا فعل على الكانفاس، وللتنقل بين الحقول أثناء التحرير، يحتاج قرارًا؛ المراجعة القديمة رصدت انتقاله إلى أزرار تلتقط Space/Enter. لا Ctrl-F6 افتراضي محجوز على Mac. |

كل source يعيد فحص enablement عند drain؛ menu قد يعرض حالة قديمة. pointer capture منفصل عن modal. لا حذف لـMenuCmd::Key قبل إثبات مسار تحرير النص، لأن native accelerator قد لا ينتج Winit keyboard event. اطلب repaint بعد command يغيّر العرض.

المفاتيح physical KeyCode، مع فصل primary عن physical Control وAlt/Shift؛ المنصة في adapter. اختبارات collisions/reserved chords ونطاق الفوكس، ولا native accelerators للمفاتيح العادية بلا primary. جدول hints لاحقًا واحد، بترتيب Control/Option/Shift/Command على Mac؛ لا اسم أداة في عنوان النافذة. احفظ S1 chords؛ اختبر تبديل tabs ضد ترتيب الطبقات قبل أي تغيير. repeats للنقل الدقيق/zoom فقط بعد التحقق من incumbent behavior.

U5 يستقبل InputEvent مملوكًا بسيطًا من Winit adapter؛ لا إنشاء Winit KeyEvent الخاص في tests. clear held keys عند focus loss، ولا release يتسرب خلال tab switch. canvas physical pixels/native scale، egui points حسب pixels_per_point الذي يشمل zoom_factor؛ تحويل مرة واحدة. الاختبارات تستخدم Platform fixture صريحًا، ولا platform cfg داخل control logic.

## K3 — الحقول والتراجع

**الحالي:** Ui::settle يلغي picker preview ويتخلص من rename/typed buffers قبل lifecycle؛ buffers مملحة بـSessionId بالفعل. E2/F1/F2 تحافظ على S1 وتذكر القيد في القبول؛ تغيير التأكيد قطعة U3-T مستقلة.

**مقترح يحتاج قرارًا قبل U3-T:** buffer يستطيع host قراءته مع document/generation/field/original/last-valid. Save أو tab switch يؤكد النص الصحيح أولًا. النص غير الصالح يمنع الطلب مع رسالة ويحفظ الفوكس؛ Escape يرجع الأصل. Close/Quit يؤكد الصحيح قبل حساب dirty وعرض الخيارات؛ Don't Save يتخلص منه فقط بعد اختيار المستخدم. يُحدث أمر S1 واختباراته في نفس القطعة. يمكن للمالك إبقاء سياسة discard الحالية بدل هذا المقترح؛ لا يعتبر الحل الجديد معتمدًا الآن.

Core وحده يملك transaction: begin(owner)/update(owner, edit)/commit(owner)/cancel(owner)، وهي API مقترحة لا موجودة. begin من owner آخر يرفض بلا overwrite في release أيضًا. الأوامر ذات begin/commit داخلي تشارك المعاملة المملوكة أو ترفض قبل التعديل؛ لا undo داخلي. update لا يغير saved checkpoint؛ commit لتغيير فعلي = undo واحد، cancel يعيد الأصل بلا تاريخ. scrub واحد أو picker Apply = undo واحد. اختبر nested begin وforeign edit أثناء preview وno-op وcancel بعد updates وsave/dirty/undo.

UI يحتفظ بتوكن الملكية وبفر الحقل لا pending document ثانٍ. canvas gesture يبقى في core؛ picker القديم adapter حتى انتقاله. focus loss يلغي preview غير المؤكد ويمسح المفاتيح؛ modal يحسم Apply/Cancel قبل foreign edit. لا dispatch لتعديل لا يملك transaction المفتوحة. سياسة التسوية النهائية تختبر قبل توصيل المستهلكين.

Editor::constrain_wh مصدر واحد للقفل؛ bounds command يحمل reference point بالفعل، فتستخدمه المرايا بلا keep_ratio مكرر. tool/view بلا undo؛ إعدادات المستند تتبع checkpoint S1. نحتفظ بسياسة nudge الحالية حتى قرار مستقل.

## K4 — الحالة والقراءات والأداء

U2-D يختار DocUi في set_tabs(active_id)، ويحذف الحالة عند إغلاق التبويب. document_switched يُستدعى بعد Save أيضًا؛ ليس مكان اختيار الهوية أو مسح search/collapse. افصل temporary invalidation عن حالة المستخدم. widget IDs مملحة بـSessionId؛ استبدال مستند داخل pristine session بنفس id يحتاج generation جديدة، لا reset مع كل حفظ.

U2-P يقيس release الحالي على Mac: صغير و10k عنصر، تحديد واحد/الكل، drag حي، idle وtab switch؛ زمن القراءة وp95 وrebuild counts والجهاز والعينة والأوامر. 360ms بالمراجعة قياس تاريخي، وP11 لا يثبت سرعة كل UI read. لا تنسخ المستند كاملًا كل frame ولا تعد بـ1ms/zero allocation بلا قياس.

invalidation يشمل session+generation وcommitted revision وselection وlive geometry. topology/labels حسب تغييراتها؛ selected rows حسب selection؛ thumbnails/الأبعاد حسب live geometry. fingerprint موثق ممكن أولًا؛ لا O(1) index cache قبل حصر mutations. القبول: اختبارات stale reads، rebuild counters ثابتة في idle، الحقول تواكب drag، ولا regression خارج تشتت baseline المقاس. budget رقمي إضافي يُثبت مع الجهاز قبل optimization. لا تجمع القياس والتحسين ونقل state في diff واحد.

## K5 — الحد الأدنى للـkit والخطوط

shell/tokens.rs مصدر قيم runtime الوحيد؛ لا tests لتزامن Markdown/mockup مع Rust. inventory للألوان error/warning/none/guide والمقاسات والأوزان والمسافات قبل تغييرها؛ لا palette أو seam جديد من v1. labels الجديدة في مصدر صغير مشترك خارج painter؛ لا إطار i18n كامل. Mono للأرقام لا الأسماء؛ casing في العرض.

| U0 minimum | المستهلك |
|---|---|
| action/icon button مع label/help/disabled reason | New/Open وRecent ثم F2 |
| list row وsection heading | Recent والحالة الفارغة؛ F2 يمدها للاسترجاع |
| Home/tab chip | Home فقط؛ نقل document tabs لاحقًا باختبارات drag/focus |
| text/notice/error/busy presentation | empty/missing/open failure؛ dialogs الحالية، بلا search field جديد |

لا switch/num_field/swatch/menu عامة قبل مستهلك محدد؛ U3-K يقدم كل control قبل بانله. disabled له سبب وsemantics؛ busy يمنع التكرار، Cancel فقط لو العملية قابلة للإلغاء؛ حدث واحد لكل إجراء ولا فعل من paint أو ghost.

الحالات: disabled يسبق on ثم pressed/hover، وfocus-visible overlay مستقل واضح فوق azure، keyboard لا لمجرد click. اختبر selected+disabled/hover/focus. switch/HUD/caret/drop تفصيلها مع مستهلكها، لا نسخ tool-on. hit target 24pt أو استثناء موثق بمسافات آمنة. النص العادي 4.5:1 والفوكس/المكونات غير النصية 3:1 على الخلفية المركبة الفعلية؛ hover/selection قد يحتاج TEXT بدل MUTED. لا FAINT لمعلومة لازمة.

U0-A يثبت نسخة fonts ومصدر IBM/checksum/OFL؛ يحفظ الرخصة ويتجنب subsetting غير المتوافق مع Reserved Font Names. لا بحث في fonts النظام. يفحص الأوزان والأرقام و⌘⌥⇧⌃ وباقي glyphs؛ الناقص icon أو fallback مرخص ومضمّن. إزالة default_fonts بعد إثبات التغطية وقياس binary فقط. spike على egui المثبت للشكل/bidi/caret/selection/clipboard/graphemes: أسماء عربية متعددة الكلمات ومختلطة وpath طويل. وجود glyphs لا يثبت bidi أو التحرير؛ failure يمنع إعلان نجاح الدعم ويعود بإصلاح محدود أو قيد صريح، دون تغيير النص المخزن أو فتح مشروع RTL كامل.

CPU testkit: warm-up بعد set_fonts، hook key→rect، alpha باسم token ونسبته، ppp=1/2، scripted hover/press/focus. goldens قليلة للمكونات؛ panels semantic/event tests بدل rect لكل حرف. widget_info/semantics حسب دعم egui الحالي؛ AccessKit في lockfile لا يثبت screen-reader support. gallery صغيرة وفحص نافذة Mac للخط/القص/الفوكس؛ لا GPU Renderer/EventLoop في headless tests ولا ادعاء أن shapes تثبت الشكل.

## K6 — البانلات والحجم والحركة

registry metadata واحد وmatch للرسم بلا Panel trait مكرر. domain له section-home؛ مرآته تفتح container وتظهر وتوسع section. overflow يحتفظ بوجهة كل domain. Window tick يعني visible/frontmost؛ اختيار hidden يفتحه ويركزه، واختيار visible يبدل ظهوره وفق قواعده. Board واحد دائم لا يغلق أو يتبوب. Start surface يحجب doc panels/shortcuts مع بقاء workspace never-empty؛ العودة تحفظ tabs/selection/dirty.

U4-S يحدد pre-layout logical points: min/preferred/max ومكان Fill واحد يحتوي Board أو آخر طفل، والتحويل إلى shares داخل boxtree فقط. Properties resizable؛ 274 و800×560 أمثلة v1 غير معتمدة. قِس chrome/rulers/recovery strip والشاشة الفعلية؛ عند نقص المساحة تطوى مناطق اختيارية مع وصول من Window. board rect يستبعد rulers؛ لا minimum أكبر من مساحة الشاشة ولا placement redesign أثناء extraction.

U3-B يملك slot table/fold order للـbar؛ U4-P placement/clamp/Fit فقط. Fit من occlusion rectangles للـbar/rail الفعليين بدل bands منافسة لـQW8. tree normalization عند mutation لا كل frame؛ ghost مع disabled input وscratch output يُرمى بلا requests.

U4-M يفصل ghost easing/fork glide/glow عن direction bar الباقي؛ ADR superseding ثم patch ledger/hash verification قبل تعديل fork. gate الحالي يثبت confinement لا patch drift. styling يضبط scroll_animation إلى none؛ line-wheel smoothing المتبقي يُقاس ويُذكر دون ادعاء صفر حركة في egui. E2 يزيل splash مع حفظ GPU startup failure. أي تعديل UI_DIRECTION معه قرار صريح؛ spec لا يعلو عليه.

Workspaces مؤجلة بعد size model: app-owned versioned schema والتحويل محصور في boxtree؛ حماية corrupt/newer files، debounced worker/flush، اختبارات clamp/monitor/Reset. لا serialize fork ولا ملف layout منافس. Accessibility لاحقة للـfull RTL وbox focus shortcuts؛ bidi أسماء U0 لا ينتظرها. double-click يستخدم egui InputOptions وقيمة OS مشتركة؛ custom helper للكانفاس/caption فقط ضمن U5، لا dependency جديدة مفترضة.

## القطع وملكية الملفات

المسارات داخل app/src إلا core المذكور. قطعة واحدة تمسك الملفات المشتركة في كل مرة؛ هذه ليست دعوة لتنفيذ متوازٍ أو تقديرات agent-days. كل قطعة تعيد فحص baseline قبل البدء.

| القطعة | الملكية والترتيب | دليل الإقفال |
|---|---|---|
| U0-A | shell/tokens.rs، font loader/assets/manifest عند الحاجة | inventory/license/coverage وbidi spike ونافذة Mac قبل تعميم الخط |
| U0-B ثم C | shell/kit وshell/mod.rs ثم test hooks/gallery؛ بعد A | minimum controls فقط؛ lib مستقل؛ scripts 1/2ppp وفحص بصري |
| E2 | start_ui.rs، ui.rs، main.rs، host.rs، app_command.rs وrecents؛ بعد minimum U0 وS5 حسب PLAN | قبول S2/S3: launch/Home/Recent/Locate/last-tab، مسار فتح واحد، remove/clear لا يحذف ملفات |
| F1 ثم F2 | host/lifecycle/workspace/UI حسب أمر S2/S3؛ لا U1 متزامن | durable Save/recovery writing ثم recover-as-copy |
| S4 ثم S6 | host/menu/lifecycle/export UI حسب أمرهما | association/export عبر AppCommand الحالي بلا انتظار framework |
| U1-A ثم B ثم C | types/request/context ثم menu table ثم keyboard adapter؛ app_command/host/menu/main/ui بالتتابع | responder/FIFO/modal/dispatch/repaint tests، الحفاظ على S1 |
| U2-O | ui.rs وrequest adapter؛ بعد U1 types وقبل panels | Op→Request ميكانيكيًا، typed drop/rename/picker targets، transient setters كما K1 |
| U2-P ثم D | Snap measurements/cache، ثم frame/DocUi extraction؛ ui/workspace وأقل core probes | K4؛ لا دمج optimization وstate migration في diff واحد |
| U3-T | core editor.rs/command.rs أولًا ثم ui field host؛ بعد قرار K3 | live span release-safe وundo/cancel/settle قبل المستهلك |
| U3-K ثم A ثم B ثم C | controls حسب الاستهلاك، Properties ثم bar mirrors ثم Layers/picker كل على حدة؛ owner واحد لـui.rs | mirrors من نفس القيم؛ rename/selection/picker tests؛ picker بعد live span |
| U4-S ثم M ثم P | size ثم ADR/vendor motion ثم placement؛ boxtree/registry/host | K6؛ clamp/Fit/tests + Mac؛ storage مؤجل |
| U5-A ثم B | router extraction بسلوك محفوظ، ثم input fixes؛ main/host/core editor | K2/cancel gesture؛ لا focus engine جديد. pinch اختياري لاحق: finite delta، clamp ≤−1، yield فوق chrome؛ smart zoom دون فعل إلى قرار |
| U6 | تنظيف بعد ثبوت المستهلكين | icon set موحد بالتدريج؛ grep word-boundary فلا يطابق Op داخل Option؛ لا doc-token parsers أو vendor-neutral standard قبل V1 |

core/editor لا يتشارك بين U3-T وU5 وF5. F5/F6 تحت الميثاق: reconciliation لفرع codex/p6-header وOWNERSHIP_MAP قبل extraction. التشخيص والأداء والflags تحت F7، بلا telemetry أو تغيير flags هنا.

## قبول التخطيط والخطوة التالية

- جسم v1 والمراجعات القديمة محفوظة؛ كل finding له تصرف وقطعة.
- STATUS/PLAN وأوامر E2/S6 متفقة على minimum U0؛ لا framework أو persistence كشرط خفي للاسترجاع.
- روابط وdiff وgates محلية قبل commit؛ تعديل Markdown لا ينتج runtime أو visual approval.
- المراجعة الذاتية ليست مستقلة؛ merge يحتاج independent review وفق القواعد. فوترة GitHub لا تستحدث شرط hosted CI جديدًا.

بعد مراجعة هذه الدفعة، التنفيذ التالي **S5-C: semantic validator** بعقد قبوله الحالي، ثم S5-D/E ثم U0-A. قرارات K3/Tab وتفاصيل HUD/switch تُعرض مع مثال عند بدء القطعة المعنية؛ لا نعيد أسئلة Home والخطوط.
