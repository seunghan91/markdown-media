---
format: pdf
version: "1.5"
pages: 15
images: 6
fonts: 34
tables: 6
---

### Providedproperattributionisprovided,Googleherebygrantspermissionto

### reproducethetablesandfiguresinthispapersolelyforuseinjournalisticor

### scholarlyworks.

## AttentionIsAllYouNeed

∗ ∗ ∗ ∗ AshishVaswani NoamShazeer NikiParmar JakobUszkoreit GoogleBrain GoogleBrain GoogleResearch GoogleResearch avaswani@google.com noam@google.com nikip@google.com usz@google.com

∗ ∗† ∗ LlionJones AidanN.Gomez ŁukaszKaiser GoogleResearch UniversityofToronto GoogleBrain llion@google.com aidan@cs.toronto.edu lukaszkaiser@google.com

∗‡ IlliaPolosukhin illia.polosukhin@gmail.com

### Abstract

Thedominantsequencetransductionmodelsarebasedoncomplexrecurrentor convolutionalneuralnetworksthatincludeanencoderandadecoder.Thebest

performingmodelsalsoconnecttheencoderanddecoderthroughanattention mechanism.Weproposeanewsimplenetworkarchitecture,theTransformer,

basedsolelyonattentionmechanisms,dispensingwithrecurrenceandconvolutions entirely.Experimentsontwomachinetranslationtasksshowthesemodelsto

besuperiorinqualitywhilebeingmoreparallelizableandrequiringsignificantly lesstimetotrain.Ourmodelachieves28.4BLEUontheWMT2014English-

to-Germantranslationtask,improvingovertheexistingbestresults,including ensembles,byover2BLEU.OntheWMT2014English-to-Frenchtranslationtask,

ourmodelestablishesanewsingle-modelstate-of-the-artBLEUscoreof41.8after trainingfor3.5daysoneightGPUs,asmallfractionofthetrainingcostsofthe

bestmodelsfromtheliterature.WeshowthattheTransformergeneralizeswellto arXiv:1706.03762v7 [cs.CL] 2 Aug 2023 othertasksbyapplyingitsuccessfullytoEnglishconstituencyparsingbothwith

largeandlimitedtrainingdata.

| ∗ | Equalcontribution.Listingorderisrandom.JakobproposedreplacingRNNswithself-attentionandstarted |
| --- | --- |
| † | WorkperformedwhileatGoogleBrain. |
| ‡ | WorkperformedwhileatGoogleResearch. |


WorkperformedwhileatGoogleResearch.

31stConferenceonNeuralInformationProcessingSystems(NIPS2017),LongBeach,CA,USA.

@[[image_1]]

@[[image_2]]

@[[image_3]]

@[[image_4]]

@[[image_5]]

@[[image_6]]

### 1Introduction

Recurrentneuralnetworks,longshort-termmemory[13]andgatedrecurrent[7]neuralnetworks inparticular,havebeenfirmlyestablishedasstateoftheartapproachesinsequencemodelingand

transductionproblemssuchaslanguagemodelingandmachinetranslation[35, 2, 5].Numerous effortshavesincecontinuedtopushtheboundariesofrecurrentlanguagemodelsandencoder-decoder

architectures[38,24,15]. Recurrentmodelstypicallyfactorcomputationalongthesymbolpositionsoftheinputandoutput sequences.Aligningthepositionstostepsincomputationtime,theygenerateasequenceofhidden

states h t ,asafunctionoftheprevioushiddenstate h t − 1 andtheinputforposition t .Thisinherently sequentialnatureprecludesparallelizationwithintrainingexamples,whichbecomescriticalatlonger

sequencelengths,asmemoryconstraintslimitbatchingacrossexamples.Recentworkhasachieved significantimprovementsincomputationalefficiencythroughfactorizationtricks[21]andconditional

computation[32],whilealsoimprovingmodelperformanceincaseofthelatter.Thefundamental constraintofsequentialcomputation,however,remains.

Attentionmechanismshavebecomeanintegralpartofcompellingsequencemodelingandtransduc- tionmodelsinvarioustasks,allowingmodelingofdependencieswithoutregardtotheirdistancein

theinputoroutputsequences[2, 19].Inallbutafewcases[27],however,suchattentionmechanisms areusedinconjunctionwitharecurrentnetwork.

InthisworkweproposetheTransformer,amodelarchitectureeschewingrecurrenceandinstead relyingentirelyonanattentionmechanismtodrawglobaldependenciesbetweeninputandoutput.

TheTransformerallowsforsignificantlymoreparallelizationandcanreachanewstateoftheartin translationqualityafterbeingtrainedforaslittleastwelvehoursoneightP100GPUs.

### 2Background

ThegoalofreducingsequentialcomputationalsoformsthefoundationoftheExtendedNeuralGPU [16],ByteNet[18]andConvS2S[9],allofwhichuseconvolutionalneuralnetworksasbasicbuilding

block,computinghiddenrepresentationsinparallelforallinputandoutputpositions.Inthesemodels, thenumberofoperationsrequiredtorelatesignalsfromtwoarbitraryinputoroutputpositionsgrows

inthedistancebetweenpositions,linearlyforConvS2SandlogarithmicallyforByteNet.Thismakes itmoredifficulttolearndependenciesbetweendistantpositions[12].IntheTransformerthisis

reducedtoaconstantnumberofoperations,albeitatthecostofreducedeffectiveresolutiondue toaveragingattention-weightedpositions,aneffectwecounteractwithMulti-HeadAttentionas

describedinsection3.2. Self-attention,sometimescalledintra-attentionisanattentionmechanismrelatingdifferentpositions ofasinglesequenceinordertocomputearepresentationofthesequence.Self-attentionhasbeen

usedsuccessfullyinavarietyoftasksincludingreadingcomprehension,abstractivesummarization, textualentailmentandlearningtask-independentsentencerepresentations[4,27,28,22].

End-to-endmemorynetworksarebasedonarecurrentattentionmechanisminsteadofsequence- alignedrecurrenceandhavebeenshowntoperformwellonsimple-languagequestionansweringand

languagemodelingtasks[34]. Tothebestofourknowledge,however,theTransformeristhefirsttransductionmodelrelying entirelyonself-attentiontocomputerepresentationsofitsinputandoutputwithoutusingsequence-

alignedRNNsorconvolution.Inthefollowingsections,wewilldescribetheTransformer,motivate self-attentionanddiscussitsadvantagesovermodelssuchas[17,18]and[9].

### 3ModelArchitecture

Mostcompetitiveneuralsequencetransductionmodelshaveanencoder-decoderstructure[5, 2, 35]. Here,theencodermapsaninputsequenceofsymbolrepresentations (x,...,x) toasequence

1. n ofcontinuousrepresentations z =(z 1,...,z n).Given z,thedecoderthengeneratesanoutput


sequence (y,...,y) ofsymbolsoneelementatatime.Ateachstepthemodelisauto-regressive 1 m [10],consumingthepreviouslygeneratedsymbolsasadditionalinputwhengeneratingthenext.

@[[image_1]]

@[[image_2]]

@[[image_3]]

@[[image_4]]

@[[image_5]]

@[[image_6]]

Figure1:TheTransformer-modelarchitecture.

TheTransformerfollowsthisoverallarchitectureusingstackedself-attentionandpoint-wise,fully connectedlayersforboththeencoderanddecoder,shownintheleftandrighthalvesofFigure1,

respectively.

3.1EncoderandDecoderStacks

| Encoder: | Theencoderiscomposedofastackof | N | =6 | identicallayers.Eachlayerhastwo |
| --- | --- | --- | --- | --- |
| thetwosub-layers,followedbylayernormalization[ |  | 1 | ].Thatis,theoutputofeachsub-layeris |  |
| LayerNorm( | x +Sublayer( | x ) isthefunctionimplementedbythesub-layer |  |  |
| layers,produceoutputsofdimension |  | . |  |  |
| Decoder: | Thedecoderisalsocomposedofastackof | N | =6 | identicallayers.Inadditiontothetwo |


attentionovertheoutputoftheencoderstack.Similartotheencoder,weemployresidualconnections aroundeachofthesub-layers,followedbylayernormalization.Wealsomodifytheself-attention

sub-layerinthedecoderstacktopreventpositionsfromattendingtosubsequentpositions.This masking,combinedwithfactthattheoutputembeddingsareoffsetbyoneposition,ensuresthatthe

predictionsforposition i candependonlyontheknownoutputsatpositionslessthan i .

3.2Attention

Anattentionfunctioncanbedescribedasmappingaqueryandasetofkey-valuepairstoanoutput, wherethequery,keys,values,andoutputareallvectors.Theoutputiscomputedasaweightedsum

@[[image_1]]

@[[image_2]]

@[[image_3]]

@[[image_4]]

@[[image_5]]

@[[image_6]]

ScaledDot-ProductAttention Multi-HeadAttention

Figure2:(left)ScaledDot-ProductAttention.(right)Multi-HeadAttentionconsistsofseveral attentionlayersrunninginparallel.

ofthevalues,wheretheweightassignedtoeachvalueiscomputedbyacompatibilityfunctionofthe querywiththecorrespondingkey.

3.2.1ScaledDot-ProductAttention

Wecallourparticularattention"ScaledDot-ProductAttention"(Figure2).Theinputconsistsof queriesandkeysofdimension d

| queriesandkeysofdimension | ,andvaluesofdimension √ |  |  |
| --- | --- | --- | --- |
| querywithallkeys,divideeachby | d | k | ,andapplyasoftmaxfunctiontoobtaintheweightsonthe |
| linearprojectionsto | v dimensions,respectively.Oneachoftheseprojectedversionsof |  |  |
| variableswithmean | .Thentheirdotproduct, |  |  |


@[[image_1]]

@[[image_2]]

@[[image_3]]

@[[image_4]]

@[[image_5]]

@[[image_6]]

outputvalues.Theseareconcatenatedandonceagainprojected,resultinginthefinalvalues,as depictedinFigure2.

Multi-headattentionallowsthemodeltojointlyattendtoinformationfromdifferentrepresentation subspacesatdifferentpositions.Withasingleattentionhead,averaginginhibitsthis.

O MultiHead(Q,K,V)=Concat(head,..., head) W 1 h

Q K V where head i =Attention(QW,KW,VW) i i i

Q d × d K d × d V d × d model k model k model v Wheretheprojectionsareparametermatrices W ∈ R , W ∈ R , W ∈ R i i i

O hd × d v model and W ∈ R .

| W O | ∈ R hd |
| --- | --- |
| = d v | = d model |
| • | In"encoder-decoderattention"layers,thequeriescomefromthepreviousdecoderlayer, |
| • | Theencodercontainsself-attentionlayers.Inaself-attentionlayerallofthekeys,values |
| • | Similarly,self-attentionlayersinthedecoderalloweachpositioninthedecodertoattendto |


informationflowinthedecodertopreservetheauto-regressiveproperty.Weimplementthis insideofscaleddot-productattentionbymaskingout(settingto −∞)allvaluesintheinput

ofthesoftmaxwhichcorrespondtoillegalconnections.SeeFigure2.

3.3Position-wiseFeed-ForwardNetworks

Inadditiontoattentionsub-layers,eachofthelayersinourencoderanddecodercontainsafully connectedfeed-forwardnetwork,whichisappliedtoeachpositionseparatelyandidentically.This

consistsoftwolineartransformationswithaReLUactivationinbetween.

FFN(x)=max(0,xW + b) W + b (2) 1 1 2 2

Whilethelineartransformationsarethesameacrossdifferentpositions,theyusedifferentparameters fromlayertolayer.Anotherwayofdescribingthisisastwoconvolutionswithkernelsize1.

Thedimensionalityofinputandoutputis d =512 ,andtheinner-layerhasdimensionality model d ff =2048 .

3.4EmbeddingsandSoftmax

Similarlytoothersequencetransductionmodels,weuselearnedembeddingstoconverttheinput tokensandoutputtokenstovectorsofdimension d .Wealsousetheusuallearnedlineartransfor- model mationandsoftmaxfunctiontoconvertthedecoderoutputtopredictednext-tokenprobabilities.In

ourmodel,wesharethesameweightmatrixbetweenthetwoembeddinglayersandthepre-softmax √ lineartransformation,similarto[30].Intheembeddinglayers,wemultiplythoseweightsby d. model@[[image_1]]

@[[image_2]]

@[[image_3]]

@[[image_4]]

@[[image_5]]

@[[image_6]]

Table1:Maximumpathlengths,per-layercomplexityandminimumnumberofsequentialoperations fordifferentlayertypes. n isthesequencelength, d istherepresentationdimension, k isthekernel

sizeofconvolutionsand r thesizeoftheneighborhoodinrestrictedself-attention.

LayerTypeComplexityperLayerSequentialMaximumPathLength Operations

2. Self-Attention O (n · d) O (1) O (1) 2


Recurrent O (n · d) O (n) O (n) 2 Convolutional O (k · n · d) O (1) O (log

k (n)) Self-Attention(restricted) O (r · n · d) O (1) O (n/r)

3.5PositionalEncoding

Sinceourmodelcontainsnorecurrenceandnoconvolution,inorderforthemodeltomakeuseofthe orderofthesequence,wemustinjectsomeinformationabouttherelativeorabsolutepositionofthe

tokensinthesequence.Tothisend,weadd"positionalencodings"totheinputembeddingsatthe bottomsoftheencoderanddecoderstacks.Thepositionalencodingshavethesamedimension d model astheembeddings,sothatthetwocanbesummed.Therearemanychoicesofpositionalencodings,

learnedandfixed[9]. Inthiswork,weusesineandcosinefunctionsofdifferentfrequencies:2. i/d model PE = sin (pos/ 10000) (pos, 2 i)
2. i/d model PE = cos (pos/ 10000) (pos, 2 i +1)


where pos isthepositionand i isthedimension.Thatis,eachdimensionofthepositionalencoding correspondstoasinusoid.Thewavelengthsformageometricprogressionfrom 2 π to 10000 · 2 π .We

chosethisfunctionbecausewehypothesizeditwouldallowthemodeltoeasilylearntoattendby relativepositions,sinceforanyfixedoffset k , PE canberepresentedasalinearfunctionof pos + k PE . pos

Wealsoexperimentedwithusinglearnedpositionalembeddings[9]instead,andfoundthatthetwo versionsproducednearlyidenticalresults(seeTable3row(E)).Wechosethesinusoidalversion

becauseitmayallowthemodeltoextrapolatetosequencelengthslongerthantheonesencountered duringtraining.

### 4WhySelf-Attention

Inthissectionwecomparevariousaspectsofself-attentionlayerstotherecurrentandconvolu- tionallayerscommonlyusedformappingonevariable-lengthsequenceofsymbolrepresentations

d (x,...,x) toanothersequenceofequallength (z,...,z),with x,z ∈ R,suchasahidden 1 n 1 n i i layerinatypicalsequencetransductionencoderordecoder.Motivatingouruseofself-attentionwe considerthreedesiderata.

Oneisthetotalcomputationalcomplexityperlayer.Anotheristheamountofcomputationthatcan beparallelized,asmeasuredbytheminimumnumberofsequentialoperationsrequired.

Thethirdisthepathlengthbetweenlong-rangedependenciesinthenetwork.Learninglong-range dependenciesisakeychallengeinmanysequencetransductiontasks.Onekeyfactoraffectingthe

abilitytolearnsuchdependenciesisthelengthofthepathsforwardandbackwardsignalshaveto traverseinthenetwork.Theshorterthesepathsbetweenanycombinationofpositionsintheinput

andoutputsequences,theeasieritistolearnlong-rangedependencies[12].Hencewealsocompare themaximumpathlengthbetweenanytwoinputandoutputpositionsinnetworkscomposedofthe

differentlayertypes. AsnotedinTable1,aself-attentionlayerconnectsallpositionswithaconstantnumberofsequentially executedoperations,whereasarecurrentlayerrequires O (n) sequentialoperations.Intermsof

computationalcomplexity,self-attentionlayersarefasterthanrecurrentlayerswhenthesequence

@[[image_1]]

@[[image_2]]

@[[image_3]]

@[[image_4]]

@[[image_5]]

@[[image_6]]

length n issmallerthantherepresentationdimensionality d ,whichismostoftenthecasewith sentencerepresentationsusedbystate-of-the-artmodelsinmachinetranslations,suchasword-piece

[38]andbyte-pair[31]representations.Toimprovecomputationalperformancefortasksinvolving verylongsequences,self-attentioncouldberestrictedtoconsideringonlyaneighborhoodofsize r in

theinputsequencecenteredaroundtherespectiveoutputposition.Thiswouldincreasethemaximum pathlengthto O (n/r).Weplantoinvestigatethisapproachfurtherinfuturework.

Asingleconvolutionallayerwithkernelwidth k<n doesnotconnectallpairsofinputandoutput positions.Doingsorequiresastackof O (n/k) convolutionallayersinthecaseofcontiguouskernels,

or O (log (n)) inthecaseofdilatedconvolutions[18],increasingthelengthofthelongestpaths k betweenanytwopositionsinthenetwork.Convolutionallayersaregenerallymoreexpensivethan

recurrentlayers,byafactorof k.Separableconvolutions[6],however,decreasethecomplexity 2 considerably,to O (k · n · d + n · d).Evenwith k = n,however,thecomplexityofaseparable

convolutionisequaltothecombinationofaself-attentionlayerandapoint-wisefeed-forwardlayer, theapproachwetakeinourmodel.

Assidebenefit,self-attentioncouldyieldmoreinterpretablemodels.Weinspectattentiondistributions fromourmodelsandpresentanddiscussexamplesintheappendix.Notonlydoindividualattention

headsclearlylearntoperformdifferenttasks,manyappeartoexhibitbehaviorrelatedtothesyntactic andsemanticstructureofthesentences.

### 5Training

Thissectiondescribesthetrainingregimeforourmodels.

5.1TrainingDataandBatching

WetrainedonthestandardWMT2014English-Germandatasetconsistingofabout4.5million sentencepairs.Sentenceswereencodedusingbyte-pairencoding[3],whichhasasharedsource-

targetvocabularyofabout37000tokens.ForEnglish-French,weusedthesignificantlylargerWMT 2014English-Frenchdatasetconsistingof36Msentencesandsplittokensintoa32000word-piece

vocabulary[38].Sentencepairswerebatchedtogetherbyapproximatesequencelength.Eachtraining batchcontainedasetofsentencepairscontainingapproximately25000sourcetokensand25000

targettokens.

5.2HardwareandSchedule

Wetrainedourmodelsononemachinewith8NVIDIAP100GPUs.Forourbasemodelsusing thehyperparametersdescribedthroughoutthepaper,eachtrainingsteptookabout0.4seconds.We

trainedthebasemodelsforatotalof100,000stepsor12hours.Forourbigmodels,(describedonthe bottomlineoftable3),steptimewas1.0seconds.Thebigmodelsweretrainedfor300,000steps

(3.5days).

5.3Optimizer

− 9 WeusedtheAdamoptimizer[20]with β =0. 9, β =0. 98 and ϵ =10.Wevariedthelearning 1 2 rateoverthecourseoftraining,accordingtotheformula:

− 0. 5 − 0. 5 − 1. 5 lrate = d · min(step _ num,step _ num · warmup _ steps) (3) model

Thiscorrespondstoincreasingthelearningratelinearlyforthefirst warmup _ steps trainingsteps, anddecreasingitthereafterproportionallytotheinversesquarerootofthestepnumber.Weused

warmup _ steps =4000 .

5.4Regularization

Weemploythreetypesofregularizationduringtraining:

@[[image_1]]

@[[image_2]]

@[[image_3]]

@[[image_4]]

@[[image_5]]

@[[image_6]]

Table2:TheTransformerachievesbetterBLEUscoresthanpreviousstate-of-the-artmodelsonthe English-to-GermanandEnglish-to-Frenchnewstest2014testsatafractionofthetrainingcost.

- Model
- ByteNet[18]23.75 Deep-Att+PosUnk[39]39.2
- GNMT+RL[38]24.639.92 ConvS2S[9]25.1640.46
- MoE[32]26.0340.56
- Deep-Att+PosUnkEnsemble[39]40.4 GNMT+RLEnsemble[38]26.3041.16
- ConvS2SEnsemble[9]26.36
- Transformer(basemodel)27.338.1 Transformer(big)


ResidualDropout Weapplydropout[33 sub-layerinputandnormalized.Inaddition,weapplydropouttothesumsoftheembeddingsandthe

positionalencodingsinboththeencoderanddecoderstacks.Forthebasemodel,weusearateof P =0 . 1 . drop

LabelSmoothing Duringtraining,weemployedlabelsmoothingofvalue hurtsperplexity,asthemodellearnstobemoreunsure,butimprovesaccuracyandBLEUscore.

### 6Results

6.1MachineTranslation

OntheWMT2014English-to-Germantranslationtask,thebigtransformermodel(Transformer(big) inTable2)outperformsthebestpreviouslyreportedmodels(includingensembles)bymorethan

BLEU,establishinganewstate-of-the-artBLEUscoreof listedinthebottomlineofTable3.Trainingtook

surpassesallpreviouslypublishedmodelsandensembles,atafractionofthetrainingcostofanyof thecompetitivemodels.

OntheWMT2014English-to-Frenchtranslationtask,ourbigmodelachievesaBLEUscoreof outperformingallofthepreviouslypublishedsinglemodels,atlessthan

previousstate-of-the-artmodel.TheTransformer(big)modeltrainedforEnglish-to-Frenchused dropoutrate P =0. 1,insteadof 0. 3. drop Forthebasemodels,weusedasinglemodelobtainedbyaveragingthelast5checkpoints,which werewrittenat10-minuteintervals.Forthebigmodels,weaveragedthelast20checkpoints.We

usedbeamsearchwithabeamsizeof 4 andlengthpenalty werechosenafterexperimentationonthedevelopmentset.Wesetthemaximumoutputlengthduring

inferencetoinputlength+ 50,butterminateearlywhenpossible[38]. Table2summarizesourresultsandcomparesourtranslationqualityandtrainingcoststoothermodel architecturesfromtheliterature.Weestimatethenumberoffloatingpointoperationsusedtotraina

modelbymultiplyingthetrainingtime,thenumberofGPUsused,andanestimateofthesustained single-precisionfloating-pointcapacityofeachGPU

6.2ModelVariations

ToevaluatetheimportanceofdifferentcomponentsoftheTransformer,wevariedourbasemodel indifferentways,measuringthechangeinperformanceonEnglish-to-Germantranslationonthe

5. Weusedvaluesof2.8,3.7,6.0and9.5TFLOPSforK80,K40,M40andP100,respectively.


BLEUTrainingCost(FLOPs) EN-DEEN-FREN-DEEN-FR

20 1 . 0 · 10 19 20 2 . 3 · 10 1 . 4 · 10 18 20 9 . 6 · 10 1 . 5 · 10

19 20 2 . 0 · 10 1 . 2 · 10

20 8 . 0 · 10 20 21 1 . 8 · 10 1 . 1 · 10 19 21 41.29 7 . 7 · 10 1 . 2 · 10

18 3 . 3 · 10 19 28.441.8 2 . 3 · 10

]totheoutputofeachsub-layer,beforeitisaddedtothe

ϵ =0. 1 [36].This ls

2 . 0 28 . 4 .Theconfigurationofthismodelis

3 . 5 dayson 8 P100GPUs.Evenourbasemodel

41 . 0 , 1 / 4 thetrainingcostofthe

α =0. 6 [38].Thesehyperparameters

5 .

@[[image_1]]

@[[image_2]]

@[[image_3]]

@[[image_4]]

@[[image_5]]

@[[image_6]]

Table3:VariationsontheTransformerarchitecture.Unlistedvaluesareidenticaltothoseofthebase model.AllmetricsareontheEnglish-to-Germantranslationdevelopmentset,newstest2013.Listed

perplexitiesareper-wordpiece,accordingtoourbyte-pairencoding,andshouldnotbecomparedto per-wordperplexities.

| train | PPLBLEUparams |
| --- | --- |
| steps | (dev)(dev) |
| 0.0 | 4.6725.3 |
| 0.2 | 5.4725.7 |


(E) positionalembeddinginsteadofsinusoids big 610244096160.3300K

developmentset,newstest2013.Weusedbeamsearchasdescribedintheprevioussection,butno checkpointaveraging.WepresenttheseresultsinTable3.

InTable3rows(A),wevarythenumberofattentionheadsandtheattentionkeyandvaluedimensions, keepingtheamountofcomputationconstant,asdescribedinSection3.2.2.Whilesingle-head

attentionis0.9BLEUworsethanthebestsetting,qualityalsodropsoffwithtoomanyheads. InTable3rows(B),weobservethatreducingtheattentionkeysize suggeststhatdeterminingcompatibilityisnoteasyandthatamoresophisticatedcompatibility

functionthandotproductmaybebeneficial.Wefurtherobserveinrows(C)and(D)that,asexpected, biggermodelsarebetter,anddropoutisveryhelpfulinavoidingover-fitting.Inrow(E)wereplaceour

sinusoidalpositionalencodingwithlearnedpositionalembeddings[resultstothebasemodel.

6.3EnglishConstituencyParsing

ToevaluateiftheTransformercangeneralizetoothertasksweperformedexperimentsonEnglish constituencyparsing.Thistaskpresentsspecificchallenges:theoutputissubjecttostrongstructural

constraintsandissignificantlylongerthantheinput.Furthermore,RNNsequence-to-sequence modelshavenotbeenabletoattainstate-of-the-artresultsinsmall-dataregimes[37].

Wetraineda4-layertransformerwith PennTreebank[25],about40Ktrainingsentences.Wealsotraineditinasemi-supervisedsetting,

usingthelargerhigh-confidenceandBerkleyParsercorporafromwithapproximately17Msentences [37].Weusedavocabularyof16KtokensfortheWSJonlysettingandavocabularyof32Ktokens

forthesemi-supervisedsetting. Weperformedonlyasmallnumberofexperimentstoselectthedropout,bothattentionandresidual (section5.4),learningratesandbeamsizeontheSection22developmentset,allotherparameters

remainedunchangedfromtheEnglish-to-Germanbasetranslationmodel.Duringinference,we

4.9225.7 4.3326.4 213

d hurtsmodelquality.This k

9],andobservenearlyidentical

d model =1024 ontheWallStreetJournal(WSJ)portionofthe

@[[image_1]]

@[[image_2]]

@[[image_3]]

@[[image_4]]

@[[image_5]]

@[[image_6]]

Table4:TheTransformergeneralizeswelltoEnglishconstituencyparsing(ResultsareonSection23 ofWSJ)

| Parser | Training | WSJ23F1 |
| --- | --- | --- |
| Vinyals&Kaiserelal.(2014)[37] | WSJonly,discriminative | 88.3 |
| Petrovetal.(2006)[29] | WSJonly,discriminative | 90.4 |
| Zhuetal.(2013)[40] | WSJonly,discriminative | 90.4 |
| Dyeretal.(2016)[8] | WSJonly,discriminative | 91.7 |
| Transformer(4layers) | WSJonly,discriminative | 91.3 |
| Zhuetal.(2013)[40] | semi-supervised | 91.3 |
| Huang&Harper(2009)[14] | semi-supervised | 91.3 |
| McCloskyetal.(2006)[26] | semi-supervised | 92.1 |
| Vinyals&Kaiserelal.(2014)[37] | semi-supervised | 92.1 |
| Transformer(4layers) | semi-supervised | 92.7 |
| Luongetal.(2015)[23] | multi-task | 93.0 |
| Dyeretal.(2016)[8] | generative | 93.3 |


increasedthemaximumoutputlengthtoinputlength+ 300 .Weusedabeamsizeof 21 and α =0 . 3 forbothWSJonlyandthesemi-supervisedsetting.

OurresultsinTable4showthatdespitethelackoftask-specifictuningourmodelperformssur- prisinglywell,yieldingbetterresultsthanallpreviouslyreportedmodelswiththeexceptionofthe

RecurrentNeuralNetworkGrammar[8]. IncontrasttoRNNsequence-to-sequencemodels[37],theTransformeroutperformstheBerkeley- Parser[29]evenwhentrainingonlyontheWSJtrainingsetof40Ksentences.

### 7Conclusion

Inthiswork,wepresentedtheTransformer,thefirstsequencetransductionmodelbasedentirelyon attention,replacingtherecurrentlayersmostcommonlyusedinencoder-decoderarchitectureswith

multi-headedself-attention. Fortranslationtasks,theTransformercanbetrainedsignificantlyfasterthanarchitecturesbased onrecurrentorconvolutionallayers.OnbothWMT2014English-to-GermanandWMT2014

English-to-Frenchtranslationtasks,weachieveanewstateoftheart.Intheformertaskourbest modeloutperformsevenallpreviouslyreportedensembles.

Weareexcitedaboutthefutureofattention-basedmodelsandplantoapplythemtoothertasks.We plantoextendtheTransformertoproblemsinvolvinginputandoutputmodalitiesotherthantextand

toinvestigatelocal,restrictedattentionmechanismstoefficientlyhandlelargeinputsandoutputs suchasimages,audioandvideo.Makinggenerationlesssequentialisanotherresearchgoalsofours.

Thecodeweusedtotrainandevaluateourmodelsisavailableat https://github.com/ tensorflow/tensor2tensor .

Acknowledgements WearegratefultoNalKalchbrennerandStephanGouwsfortheirfruitful comments,correctionsandinspiration.

### References

[1] JimmyLeiBa,JamieRyanKiros,andGeoffreyEHinton.Layernormalization. arXivpreprint arXiv:1607.06450,2016.

[2] DzmitryBahdanau,KyunghyunCho,andYoshuaBengio.Neuralmachinetranslationbyjointly learningtoalignandtranslate. CoRR,abs/1409.0473,2014.

[3] DennyBritz,AnnaGoldie,Minh-ThangLuong,andQuocV.Le.Massiveexplorationofneural machinetranslationarchitectures. CoRR,abs/1703.03906,2017.

[4] JianpengCheng,LiDong,andMirellaLapata.Longshort-termmemory-networksformachine reading. arXivpreprintarXiv:1601.06733,2016.

@[[image_1]]

@[[image_2]]

@[[image_3]]

@[[image_4]]

@[[image_5]]

@[[image_6]]

[5] KyunghyunCho,BartvanMerrienboer,CaglarGulcehre,FethiBougares,HolgerSchwenk, andYoshuaBengio.Learningphraserepresentationsusingrnnencoder-decoderforstatistical

machinetranslation. CoRR ,abs/1406.1078,2014.

[6] FrancoisChollet.Xception:Deeplearningwithdepthwiseseparableconvolutions. arXiv preprintarXiv:1610.02357,2016.

[7] JunyoungChung,ÇaglarGülçehre,KyunghyunCho,andYoshuaBengio.Empiricalevaluation ofgatedrecurrentneuralnetworksonsequencemodeling. CoRR,abs/1412.3555,2014.

[8] ChrisDyer,AdhigunaKuncoro,MiguelBallesteros,andNoahA.Smith.Recurrentneural networkgrammars.In Proc.ofNAACL,2016.

[9] JonasGehring,MichaelAuli,DavidGrangier,DenisYarats,andYannN.Dauphin.Convolu- tionalsequencetosequencelearning. arXivpreprintarXiv:1705.03122v2,2017.

[10] AlexGraves.Generatingsequenceswithrecurrentneuralnetworks. arXivpreprint arXiv:1308.0850,2013.

[11] KaimingHe,XiangyuZhang,ShaoqingRen,andJianSun.Deepresiduallearningforim- agerecognition.In ProceedingsoftheIEEEConferenceonComputerVisionandPattern

Recognition ,pages770–778,2016.

[12] SeppHochreiter,YoshuaBengio,PaoloFrasconi,andJürgenSchmidhuber.Gradientflowin recurrentnets:thedifficultyoflearninglong-termdependencies,2001.

[13] SeppHochreiterandJürgenSchmidhuber.Longshort-termmemory. Neuralcomputation, 9(8):1735–1780,1997.

[14] ZhongqiangHuangandMaryHarper.Self-trainingPCFGgrammarswithlatentannotations acrosslanguages.In Proceedingsofthe2009ConferenceonEmpiricalMethodsinNatural

LanguageProcessing ,pages832–841.ACL,August2009.

[15] RafalJozefowicz,OriolVinyals,MikeSchuster,NoamShazeer,andYonghuiWu.Exploring thelimitsoflanguagemodeling. arXivpreprintarXiv:1602.02410,2016.

[16] ŁukaszKaiserandSamyBengio.Canactivememoryreplaceattention?In AdvancesinNeural InformationProcessingSystems,(NIPS),2016.

[17] ŁukaszKaiserandIlyaSutskever.NeuralGPUslearnalgorithms.In InternationalConference onLearningRepresentations(ICLR),2016.

[18] NalKalchbrenner,LasseEspeholt,KarenSimonyan,AaronvandenOord,AlexGraves,andKo- rayKavukcuoglu.Neuralmachinetranslationinlineartime. arXivpreprintarXiv:1610.10099v2,

2017.


[19] YoonKim,CarlDenton,LuongHoang,andAlexanderM.Rush.Structuredattentionnetworks. In InternationalConferenceonLearningRepresentations,2017.

[20] DiederikKingmaandJimmyBa.Adam:Amethodforstochasticoptimization.In ICLR,2015.

[21] OleksiiKuchaievandBorisGinsburg.FactorizationtricksforLSTMnetworks. arXivpreprint arXiv:1703.10722,2017.

[22] ZhouhanLin,MinweiFeng,CiceroNogueiradosSantos,MoYu,BingXiang,Bowen Zhou,andYoshuaBengio.Astructuredself-attentivesentenceembedding. arXivpreprint

arXiv:1703.03130 ,2017.

[23] Minh-ThangLuong,QuocV.Le,IlyaSutskever,OriolVinyals,andLukaszKaiser.Multi-task sequencetosequencelearning. arXivpreprintarXiv:1511.06114,2015.

[24] Minh-ThangLuong,HieuPham,andChristopherDManning.Effectiveapproachestoattention- basedneuralmachinetranslation. arXivpreprintarXiv:1508.04025,2015.

@[[image_1]]

@[[image_2]]

@[[image_3]]

@[[image_4]]

@[[image_5]]

@[[image_6]]

[25] MitchellPMarcus,MaryAnnMarcinkiewicz,andBeatriceSantorini.Buildingalargeannotated corpusofenglish:Thepenntreebank. Computationallinguistics,19(2):313–330,1993.

[26] DavidMcClosky,EugeneCharniak,andMarkJohnson.Effectiveself-trainingforparsing.In ProceedingsoftheHumanLanguageTechnologyConferenceoftheNAACL,MainConference,

pages152–159.ACL,June2006.

[27] AnkurParikh,OscarTäckström,DipanjanDas,andJakobUszkoreit.Adecomposableattention model.In EmpiricalMethodsinNaturalLanguageProcessing,2016.

[28] RomainPaulus,CaimingXiong,andRichardSocher.Adeepreinforcedmodelforabstractive summarization. arXivpreprintarXiv:1705.04304,2017.

[29] SlavPetrov,LeonBarrett,RomainThibaux,andDanKlein.Learningaccurate,compact, andinterpretabletreeannotation.In Proceedingsofthe21stInternationalConferenceon

ComputationalLinguisticsand44thAnnualMeetingoftheACL ,pages433–440.ACL,July 2006.

[30] OfirPressandLiorWolf.Usingtheoutputembeddingtoimprovelanguagemodels. arXiv preprintarXiv:1608.05859,2016.

[31] RicoSennrich,BarryHaddow,andAlexandraBirch.Neuralmachinetranslationofrarewords withsubwordunits. arXivpreprintarXiv:1508.07909,2015.

[32] NoamShazeer,AzaliaMirhoseini,KrzysztofMaziarz,AndyDavis,QuocLe,GeoffreyHinton, andJeffDean.Outrageouslylargeneuralnetworks:Thesparsely-gatedmixture-of-experts

layer. arXivpreprintarXiv:1701.06538 ,2017.

[33] NitishSrivastava,GeoffreyEHinton,AlexKrizhevsky,IlyaSutskever,andRuslanSalakhutdi- nov.Dropout:asimplewaytopreventneuralnetworksfromoverfitting. JournalofMachine

LearningResearch,15(1):1929–1958,2014.

[34] SainbayarSukhbaatar,ArthurSzlam,JasonWeston,andRobFergus.End-to-endmemory networks.InC.Cortes,N.D.Lawrence,D.D.Lee,M.Sugiyama,andR.Garnett,editors,

AdvancesinNeuralInformationProcessingSystems28 ,pages2440–2448.CurranAssociates, Inc.,2015.

[35] IlyaSutskever,OriolVinyals,andQuocVVLe.Sequencetosequencelearningwithneural networks.In AdvancesinNeuralInformationProcessingSystems,pages3104–3112,2014.

[36] ChristianSzegedy,VincentVanhoucke,SergeyIoffe,JonathonShlens,andZbigniewWojna. Rethinkingtheinceptionarchitectureforcomputervision. CoRR,abs/1512.00567,2015.

[37] Vinyals&Kaiser,Koo,Petrov,Sutskever,andHinton.Grammarasaforeignlanguage.In AdvancesinNeuralInformationProcessingSystems,2015.

[38] YonghuiWu,MikeSchuster,ZhifengChen,QuocVLe,MohammadNorouzi,Wolfgang Macherey,MaximKrikun,YuanCao,QinGao,KlausMacherey,etal.Google’sneuralmachine

translationsystem:Bridgingthegapbetweenhumanandmachinetranslation. arXivpreprint arXiv:1609.08144 ,2016.

[39] JieZhou,YingCao,XuguangWang,PengLi,andWeiXu.Deeprecurrentmodelswith fast-forwardconnectionsforneuralmachinetranslation. CoRR,abs/1606.04199,2016.

[40] MuhuaZhu,YueZhang,WenliangChen,MinZhang,andJingboZhu.Fastandaccurate shift-reduceconstituentparsing.In Proceedingsofthe51stAnnualMeetingoftheACL(Volume

1:LongPapers),pages434–443.ACL,August2013.

@[[image_1]]

@[[image_2]]

@[[image_3]]

@[[image_4]]

@[[image_5]]

@[[image_6]]

### AttentionVisualizations

Figure3:Anexampleoftheattentionmechanismfollowinglong-distancedependenciesinthe encoderself-attentioninlayer5of6.Manyoftheattentionheadsattendtoadistantdependencyof

theverb‘making’,completingthephrase‘making...moredifficult’.Attentionshereshownonlyfor theword‘making’.Differentcolorsrepresentdifferentheads.Bestviewedincolor.

@[[image_1]]

@[[image_2]]

@[[image_3]]

@[[image_4]]

@[[image_5]]

@[[image_6]]

Figure4:Twoattentionheads,alsoinlayer5of6,apparentlyinvolvedinanaphoraresolution.Top: Fullattentionsforhead5.Bottom:Isolatedattentionsfromjusttheword‘its’forattentionheads5

and6.Notethattheattentionsareverysharpforthisword.

@[[image_1]]

@[[image_2]]

@[[image_3]]

@[[image_4]]

@[[image_5]]

@[[image_6]]

Figure5:Manyoftheattentionheadsexhibitbehaviourthatseemsrelatedtothestructureofthe sentence.Wegivetwosuchexamplesabove,fromtwodifferentheadsfromtheencoderself-attention

atlayer5of6.Theheadsclearlylearnedtoperformdifferenttasks.

@[[image_1]]

@[[image_2]]

@[[image_3]]

@[[image_4]]

@[[image_5]]

@[[image_6]]

## Images

- image_1.raw (1520x2239, RAW)
- image_2.raw (1520x2239, RAW)
- image_3.raw (445x884, RAW)
- image_4.raw (835x1282, RAW)
- image_5.raw (445x884, RAW)
- image_6.raw (835x1282, RAW)

## Font Styles

- FZOJEB+Arial-BoldMT (Bold)
- IYXBUV+Arial-BoldMT (Bold)
- RZEDQD+Arial-BoldMT (Bold)
- SKOJEB+Arial-BoldMT (Bold)
- TWYPSZ+Arial-BoldMT (Bold)

