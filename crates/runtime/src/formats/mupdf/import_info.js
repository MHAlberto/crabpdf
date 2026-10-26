// Only this bundled program is evaluated; document HTML/PDF scripts are not executed.
var result={title:"",author:"",outline:[],pages:0};
function walk(doc,items,depth,offset) {
    if(depth>32 || result.outline.length>=10000)return;
    for(var i=0;i<items.length;i++) {
        var item=items[i],page=-1;
        if(item.uri)try{page=doc.resolveLink(item.uri);}catch(error){}
        result.outline.push({title:item.title||"",page:page>=0?page+offset:null,depth:depth});
        if(item.down)walk(doc,item.down,depth+1,offset);
    }
}
for(var i=0;i<scriptArgs.length;i++) {
    var doc=Document.openDocument(scriptArgs[i]);
    doc.layout(612,792,12);
    var title=doc.getMetaData("info:Title")||"";
    if(i===0){result.title=title;result.author=doc.getMetaData("info:Author")||"";}
    if(scriptArgs.length>1)result.outline.push({title:title||scriptArgs[i].replace(/^.*[\/\\]/,""),page:result.pages,depth:0});
    walk(doc,doc.loadOutline(),scriptArgs.length>1?1:0,result.pages);
    result.pages+=doc.countPages();
}
print(JSON.stringify(result));
